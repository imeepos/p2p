"""Byte-recording fake rendezvous server: learn the exact wire bytes a real
p2p-base node sends as a rendezvous CLIENT.

Accepts QUIC with an identity cert (the dialing node verifies OID/SPKI),
then hex-dumps every inbound stream byte-wise, tagging frame boundaries.
Never responds: the client's retries give us repeated samples.

Run: python3 fake_rz_server.py --seed SEED --listen 127.0.0.1:PORT
"""

import argparse
import asyncio
import sys

import quic_node
from framing import decode_varint
from identity import load_identity


def pretty(data: bytes) -> str:
    out = []
    pos = 0
    while pos < len(data):
        try:
            length, nxt = decode_varint(data, pos)
        except Exception:  # noqa: BLE001
            out.append("?? %s" % data[pos:].hex())
            break
        body_end = nxt + length
        out.append("[len=%d %s]" % (length, data[pos:body_end].hex()))
        pos = body_end
    return " ".join(out) if out else "(empty)"


async def dump_stream(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
    peer = writer.get_extra_info("peername")
    print("STREAM-OPEN from %s" % (peer,))
    data = b""
    while True:
        try:
            chunk = await asyncio.wait_for(reader.read(4096), timeout=15.0)
        except asyncio.TimeoutError:
            break
        if not chunk:
            break
        data += chunk
        print("  +%dB total=%d: %s" % (len(chunk), len(data), pretty(data)))
    print("STREAM-CLOSE total=%d: %s" % (len(data), pretty(data)))


async def main_async(args) -> int:
    ident, _ = load_identity(args.seed)
    host, port = args.listen.rsplit(":", 1)
    handle = await quic_node.listen(host, int(port), ident, dump_stream)
    print("fake rendezvous server listening on %s:%s peer=%s" % (host, port, ident.peer_id))
    await asyncio.sleep(args.lifetime)
    handle.close()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", required=True)
    parser.add_argument("--listen", required=True)
    parser.add_argument("--lifetime", type=int, default=120)
    args = parser.parse_args()
    try:
        return asyncio.run(asyncio.wait_for(main_async(args), timeout=120))
    except asyncio.TimeoutError:
        print("(probe window ended)")
        return 0
    except Exception as exc:  # noqa: BLE001
        print("fake server error: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
