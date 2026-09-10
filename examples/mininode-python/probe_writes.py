"""Isolate the ping failure: write pattern (single vs split) x stream position.

A: fresh conn, 1st stream, single write of [protoID frame + nonce frame]
B: same  conn, 2nd stream, split writes (protoID, drain, nonce, drain)
C: fresh conn #2, 1st stream, split writes
"""

import argparse
import asyncio
import os
import sys

import ping as ping_proto
import quic_node
from framing import encode_varint, read_frame
from identity import load_identity


async def single_write(handle) -> str:
    reader, writer = await handle.protocol.create_stream()
    nonce = os.urandom(8)
    blob = encode_varint(len(ping_proto.PING_PROTOCOL)) + ping_proto.PING_PROTOCOL \
        + encode_varint(len(nonce)) + nonce
    writer.write(blob)
    await writer.drain()
    data = await asyncio.wait_for(reader.read(4096), timeout=2.0)
    return "ECHO %s" % data.hex() if data else "EOF"


async def split_write(handle) -> str:
    reader, writer = await handle.protocol.create_stream()
    nonce = os.urandom(8)
    writer.write(encode_varint(len(ping_proto.PING_PROTOCOL)) + ping_proto.PING_PROTOCOL)
    await writer.drain()
    await asyncio.sleep(0.2)
    try:
        writer.write(encode_varint(len(nonce)) + nonce)
        await writer.drain()
    except AssertionError as exc:
        return "RESET-ON-SECOND-WRITE (%s)" % exc
    data = await asyncio.wait_for(reader.read(4096), timeout=2.0)
    return "ECHO %s" % data.hex() if data else "EOF"


async def main_async(args) -> int:
    ident, _ = load_identity(args.seed)
    host, port = args.bootstrap.rsplit("/u", 1)

    handle1 = await quic_node.dial(host, int(port), ident, args.peer)
    try:
        print("A single-write stream1 :", await single_write(handle1))
        print("B split-write  stream2 :", await split_write(handle1))
    finally:
        await handle1.close()

    handle2 = await quic_node.dial(host, int(port), ident, args.peer)
    try:
        print("C split-write  freshconn:", await split_write(handle2))
    finally:
        await handle2.close()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", required=True)
    parser.add_argument("--bootstrap", required=True)
    parser.add_argument("--peer", required=True)
    args = parser.parse_args()
    try:
        return asyncio.run(asyncio.wait_for(main_async(args), timeout=60))
    except Exception as exc:  # noqa: BLE001
        print("probe error: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
