"""Decisive rendezvous link experiments, each on a fresh connection.

E1: protocol-ID frame only, then FIN          -> who consumes the ID frame?
E2: protocol-ID frame + minimal Query          -> does a Response ever come?
E3: minimal Query WITHOUT protocol-ID frame    -> is the stream pre-stripped?
E4: protocol-ID frame + u16be(minimal Query)   -> alt length prefix
Each attempt prints a timestamp for correlation with daemon.log.
"""

import argparse
import asyncio
import struct
import sys
import time

import pbcodec as pb
import quic_node
import rendezvous as rz
from framing import decode_varint, encode_varint, read_frame
from identity import load_identity

QUERY_EMPTY_NS = b"\x0a\x00"  # field 1 (namespace) = ""


def frame_varint(body: bytes) -> bytes:
    return encode_varint(len(body)) + body


def frame_u16be(body: bytes) -> bytes:
    return struct.pack(">H", len(body)) + body


async def attempt(host, port, ident, peer_id, payload: bytes, note: str, fin: bool):
    stamp = time.strftime("%H:%M:%S")
    handle = await quic_node.dial(host, port, ident, peer_id)
    try:
        reader, writer = await handle.protocol.create_stream()
        writer.write(payload)
        await writer.drain()
        if fin:
            writer.write_eof()
        try:
            data = await asyncio.wait_for(reader.read(4096), timeout=2.0)
        except asyncio.TimeoutError:
            print("%s %s -> TIMEOUT" % (stamp, note))
            return
        if not data:
            print("%s %s -> EOF" % (stamp, note))
            return
        try:
            length, pos = decode_varint(data)
            resp = pb.decode_response(data[pos : pos + length])
            print("%s %s -> RESPONSE error=%r" % (stamp, note, resp.error))
        except Exception:  # noqa: BLE001
            print("%s %s -> DATA %s" % (stamp, note, data.hex()[:80]))
    finally:
        await handle.close()


async def main_async(args) -> int:
    ident, _ = load_identity(args.seed)
    host, port = args.bootstrap.rsplit("/u", 1)
    proto_frame = frame_varint(rz.RENDEZVOUS_PROTOCOL)
    steps = [
        ("E1 protoID+FIN", proto_frame, True),
        ("E2 protoID+query", proto_frame + frame_varint(QUERY_EMPTY_NS), False),
        ("E3 query only", frame_varint(QUERY_EMPTY_NS), False),
        ("E4 protoID+u16be(query)", proto_frame + frame_u16be(QUERY_EMPTY_NS), False),
    ]
    for note, payload, fin in steps:
        try:
            await attempt(host, int(port), ident, args.peer, payload, note, fin)
        except Exception as exc:  # noqa: BLE001
            print("%s %s -> FAIL %r" % (time.strftime("%H:%M:%S"), note, exc))
        await asyncio.sleep(0.4)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", required=True)
    parser.add_argument("--bootstrap", required=True)
    parser.add_argument("--peer", required=True)
    args = parser.parse_args()
    try:
        return asyncio.run(asyncio.wait_for(main_async(args), timeout=120))
    except Exception as exc:  # noqa: BLE001
        print("probe error: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
