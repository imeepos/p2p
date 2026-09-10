"""Black-box framing prober: discover the actual stream framing of a live
p2p-base node without reading its source.

Tries several (protocol-ID framing, payload framing) combinations against
/p2p-base/ping/1 and classifies the node's reaction:
  ECHO  - got a byte-exact reply (framing understood)
  RESET - stream reset by remote
  EOF   - clean FIN, no data
  TIMEOUT - nothing

Run: python3 probe_framing.py --bootstrap 127.0.0.1/uPORT --peer <PEERID>
"""

import argparse
import asyncio
import sys

import ping as ping_proto
import quic_node
from framing import encode_varint, read_frame
from identity import Identity, load_identity

import struct


def frames_varint(proto: bytes, nonce: bytes) -> bytes:
    return encode_varint(len(proto)) + proto + encode_varint(len(nonce)) + nonce


def frames_varint_u32be(proto: bytes, nonce: bytes) -> bytes:
    return encode_varint(len(proto)) + proto + struct.pack(">I", len(nonce)) + nonce


def frames_u32be_raw(proto: bytes, nonce: bytes) -> bytes:
    return proto + struct.pack(">I", len(nonce)) + nonce


def frames_u32be_both(proto: bytes, nonce: bytes) -> bytes:
    return (
        struct.pack(">I", len(proto)) + proto
        + struct.pack(">I", len(nonce)) + nonce
    )


def frames_varint_raw(proto: bytes, nonce: bytes) -> bytes:
    return proto + encode_varint(len(nonce)) + nonce


CANDIDATES = [
    ("varint+varint(docs)", frames_varint),
    ("varint+u32be", frames_varint_u32be),
    ("raw+u32be", frames_u32be_raw),
    ("u32be+u32be", frames_u32be_both),
    ("raw+varint", frames_varint_raw),
]


async def try_one(handle, name, builder) -> str:
    reader, writer = await handle.protocol.create_stream()
    nonce = b"\x01\x02\x03\x04"
    try:
        writer.write(builder(ping_proto.PING_PROTOCOL, nonce))
        await writer.drain()
    except Exception as exc:  # noqa: BLE001
        return "WRITE-FAIL %r" % exc
    try:
        data = await asyncio.wait_for(reader.read(4096), timeout=2.0)
    except asyncio.TimeoutError:
        writer.write_eof()
        return "TIMEOUT"
    except Exception as exc:  # noqa: BLE001
        return "ERR %r" % exc
    if data == b"":
        return "EOF"
    if data == nonce:
        return "ECHO"
    if data == nonce[: len(data)]:
        return "PARTIAL-ECHO"
    return "DATA %s" % data.hex()


async def main_async(args) -> int:
    ident, _ = load_identity(args.seed)
    host, port = args.bootstrap.rsplit("/u", 1) if "/u" in args.bootstrap else args.bootstrap.rsplit(":", 1)
    handle = await quic_node.dial(host, int(port), ident, args.peer)
    print("connected; probing framings")
    try:
        for name, builder in CANDIDATES:
            try:
                verdict = await try_one(handle, name, builder)
            except Exception as exc:  # noqa: BLE001
                verdict = "STREAM-FAIL %r" % exc
            print("%-24s -> %s" % (name, verdict))
    finally:
        await handle.close()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", required=True)
    parser.add_argument("--bootstrap", required=True)
    parser.add_argument("--peer", required=True)
    args = parser.parse_args()
    try:
        return asyncio.run(asyncio.wait_for(main_async(args), timeout=90))
    except Exception as exc:  # noqa: BLE001
        print("probe error: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
