"""Bisect the rendezvous link's frame size limit, one attempt per step.

Each step opens a fresh connection and sends:
  varint("/p2p-base/rendezvous/1") + varint(len(body)) + body
body = unknown protobuf field (tag 0x7A, len, P bytes of 0x2A) with no kind
-> a healthy link must answer Response "missing request kind".
Then the operator diffs daemon.log to correlate the server's verdict.
"""

import argparse
import asyncio
import sys

import pbcodec as pb
import quic_node
import rendezvous as rz
from framing import encode_varint, read_frame
from identity import load_identity


def body_with_padding(pad_len: int) -> bytes:
    # tag 0x7A = field 15, wire type 2 (unknown field, must be ignored)
    return bytes([0x7A]) + encode_varint(pad_len) + b"\x2a" * pad_len


async def attempt(host, port, ident, peer_id, body, timeout=2.0):
    handle = await quic_node.dial(host, port, ident, peer_id)
    try:
        reader, writer = await handle.protocol.create_stream()
        writer.write(encode_varint(len(rz.RENDEZVOUS_PROTOCOL)) + rz.RENDEZVOUS_PROTOCOL
                     + encode_varint(len(body)) + body)
        await writer.drain()
        data = await asyncio.wait_for(reader.read(4096), timeout=timeout)
        if not data:
            return "EOF"
        try:
            from framing import decode_varint

            length, pos = decode_varint(data)
            if pos + length <= len(data):
                resp = pb.decode_response(data[pos : pos + length])
                return "RESPONSE error=%r" % resp.error
        except Exception:  # noqa: BLE001
            pass
        return "DATA %s" % data.hex()[:60]
    finally:
        await handle.close()


async def main_async(args) -> int:
    ident, _ = load_identity(args.seed)
    host, port = args.bootstrap.rsplit("/u", 1)
    sizes = [int(s) for s in args.sizes.split(",")]
    for pad in sizes:
        body = body_with_padding(pad)
        verdict = await attempt(host, int(port), ident, args.peer, body)
        print("body=%dB -> %s" % (len(body), verdict))
        await asyncio.sleep(0.3)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", required=True)
    parser.add_argument("--bootstrap", required=True)
    parser.add_argument("--peer", required=True)
    parser.add_argument("--sizes", default="20,40,60,64,80,100,126,127,128,129,170")
    args = parser.parse_args()
    try:
        return asyncio.run(asyncio.wait_for(main_async(args), timeout=240))
    except Exception as exc:  # noqa: BLE001
        print("probe error: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
