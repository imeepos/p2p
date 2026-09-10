"""/p2p-base/rendezvous/1 client operations.

Docs: docs/protocol/specs/rendezvous.md, node-lifecycle.md section 1.2.
One request opens one stream. Wire facts (interop-probed against the
reference node, registered in SPEC-GAPS.md):
- The stream's first frame is the protocol ID, varint-framed like every
  stream (wire-format.md section 8); the swarm strips it.
- The rendezvous link itself frames each Request/Response with a 4-byte
  BIG-ENDIAN length prefix (u32be), NOT the stream-layer LEB128 varint.
  This diverges from wire-format.md section 6 and was confirmed by
  byte-capturing the reference client (fake_rz_server.py).
Error semantics: Response.error empty string = success; any non-empty
error is opaque text (spec section 4).
"""

import hashlib
import struct
import time
from typing import List, Optional

import pbcodec as pb
from framing import MAX_FRAME_SIZE, encode_frame, read_exact_or_eof, write_raw
from identity import Identity

RENDEZVOUS_PROTOCOL = b"/p2p-base/rendezvous/1"
DEFAULT_TTL_SECS = 60
MAX_ADDRS = 32
MAX_NAMESPACE_LEN = 64
LEN_PREFIX_SIZE = 4


class RendezvousError(Exception):
    """Server-side rejection carried in Response.error."""


async def send_request(writer, request: bytes) -> None:
    await write_raw(writer, struct.pack(">I", len(request)) + request)


async def recv_response(reader) -> Optional[pb.Response]:
    prefix = await read_exact_or_eof(reader, LEN_PREFIX_SIZE)
    if prefix is None:
        return None
    length = struct.unpack(">I", prefix)[0]
    if length > MAX_FRAME_SIZE:
        raise RendezvousError("response length %d exceeds frame cap" % length)
    body = await read_exact_or_eof(reader, length)
    if body is None:
        return None
    return pb.decode_response(body)


def _encode_addrs(addrs: List[pb.AddrMsg]) -> List[bytes]:
    if len(addrs) > MAX_ADDRS:
        raise ValueError("too many addresses: %d > %d" % (len(addrs), MAX_ADDRS))
    return [pb.encode_addrmsg(a.quic, a.ip, a.port) for a in addrs]


async def register(
    reader_writer,
    ident: Identity,
    namespace: str,
    addrs: List[pb.AddrMsg],
    ttl_secs: int = DEFAULT_TTL_SECS,
    issued_at: Optional[int] = None,
) -> Optional[pb.Response]:
    """Sign and send a Register request; returns the server Response."""
    reader, writer = reader_writer
    if not namespace or len(namespace.encode("utf-8")) > MAX_NAMESPACE_LEN:
        raise ValueError("namespace must be 1..64 bytes")
    issued_at = issued_at if issued_at is not None else int(time.time())
    peer_id = hashlib.sha256(ident.pubkey).digest()  # raw digest, not base58
    addr_frames = _encode_addrs(addrs)
    payload = pb.encode_signed_fields(
        namespace, peer_id, addr_frames, ttl_secs, issued_at
    )
    sig = ident.sign(payload)
    request = pb.encode_request_register(
        pb.encode_register(
            namespace, peer_id, ident.pubkey, addr_frames,
            ttl_secs, sig, issued_at,
        )
    )
    await write_raw(writer, encode_frame(RENDEZVOUS_PROTOCOL))
    await send_request(writer, request)
    return await recv_response(reader)


async def query(
    reader_writer,
    namespace: str,
    peer_id: Optional[bytes] = None,
) -> Optional[pb.Response]:
    """Query a namespace (peer_id=None) or one exact peer (32-byte digest)."""
    reader, writer = reader_writer
    if not namespace:
        raise ValueError("namespace must be non-empty")
    await write_raw(writer, encode_frame(RENDEZVOUS_PROTOCOL))
    await send_request(writer, pb.encode_request_query(pb.encode_query(namespace, peer_id)))
    return await recv_response(reader)


def expect_success(response: Optional[pb.Response], step: str) -> pb.Response:
    if response is None:
        raise RendezvousError("%s: server closed the link without a Response" % step)
    if response.error:
        raise RendezvousError("%s rejected: %s" % (step, response.error))
    return response
