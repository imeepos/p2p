"""Black-box rendezvous framing prober.

The swarm-level protocol ID frame is varint-framed (proven by ping echo).
This probe keeps that and varies ONLY the framing of the protobuf Request
frame, to find what the rendezvous server link actually reads:
  varint | u16be | u32be | raw

A Response (any) proves the framing; classification mirrors probe_framing.
"""

import argparse
import asyncio
import struct
import sys
import time

import pbcodec as pb
import quic_node
import rendezvous as rz
from framing import encode_varint, read_frame
from identity import Identity, load_identity


def build_register(ident: Identity, namespace: str, host: str, port: int) -> bytes:
    issued_at = int(time.time())
    import hashlib

    peer_id = hashlib.sha256(ident.pubkey).digest()
    addr = pb.encode_addrmsg(True, host, port)
    payload = pb.encode_signed_fields(namespace, peer_id, [addr], 60, issued_at)
    return pb.encode_register(
        namespace, peer_id, ident.pubkey, [addr], 60, ident.sign(payload), issued_at
    )


def wrap_varint(data: bytes) -> bytes:
    return encode_varint(len(data)) + data


def wrap_u16be(data: bytes) -> bytes:
    return struct.pack(">H", len(data)) + data


def wrap_u32be(data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + data


CANDIDATES = [
    ("varint", wrap_varint),
    ("u16be", wrap_u16be),
    ("u32be", wrap_u32be),
    ("raw", lambda data: data),
]


async def main_async(args) -> int:
    ident, _ = load_identity(args.seed)
    host, port = args.bootstrap.rsplit("/u", 1)
    handle = await quic_node.dial(host, int(port), ident, args.peer)
    register = build_register(ident, args.namespace, "127.0.0.1", args.my_port)
    print("connected; register message is %d bytes" % len(register))
    try:
        for name, wrap in CANDIDATES:
            reader, writer = await handle.protocol.create_stream()
            await writer.drain() if False else None
            writer.write(wrap_varint(rz.RENDEZVOUS_PROTOCOL) + wrap(register))
            try:
                await writer.drain()
            except Exception as exc:  # noqa: BLE001
                print("%-8s -> DRAIN-FAIL %r" % (name, exc))
                continue
            try:
                data = await asyncio.wait_for(reader.read(4096), timeout=2.0)
            except asyncio.TimeoutError:
                print("%-8s -> TIMEOUT" % name)
                continue
            except Exception as exc:  # noqa: BLE001
                print("%-8s -> ERR %r" % (name, exc))
                continue
            if data == b"":
                print("%-8s -> EOF" % name)
                continue
            # try to parse a Response out of what we got
            for label, blob in (("as-frame", data),):
                try:
                    if label == "as-frame":
                        # varint-framed response?
                        from framing import decode_varint

                        length, pos = decode_varint(data)
                        if pos + length <= len(data):
                            resp = pb.decode_response(data[pos : pos + length])
                            print("%-8s -> RESPONSE(varint) error=%r peers=%d"
                                  % (name, resp.error, len(resp.peers)))
                            break
                        resp = pb.decode_response(data)
                        print("%-8s -> RESPONSE(raw) error=%r peers=%d"
                              % (name, resp.error, len(resp.peers)))
                        break
                except Exception:
                    print("%-8s -> DATA %s" % (name, data.hex()[:120]))
                    break
    finally:
        await handle.close()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", required=True)
    parser.add_argument("--bootstrap", required=True)
    parser.add_argument("--peer", required=True)
    parser.add_argument("--namespace", default="iv1-probe")
    parser.add_argument("--my-port", type=int, default=40000)
    args = parser.parse_args()
    try:
        return asyncio.run(asyncio.wait_for(main_async(args), timeout=90))
    except Exception as exc:  # noqa: BLE001
        print("probe error: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
