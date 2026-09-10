"""Handwritten protobuf codec for /p2p-base/rendezvous/1 messages.

Docs: docs/protocol/specs/rendezvous.md section 2, node-lifecycle.md section 1.2.
Message shapes:
  AddrMsg{1:quic bool, 2:ip string, 3:port uint32}
  Register{1:namespace str, 2:peer_id bytes, 3:pubkey bytes, 4:addrs rep,
           5:ttl_secs uint32, 6:sig bytes, 7:issued_at uint64}
  Query{1:namespace str, 2:peer_id bytes(optional)}
  PeerEntry{1:peer_id bytes, 2:addrs rep}
  Response{1:error str, 2:peers rep}
  Request{1:Register, 2:Query}
  SignedFields{1:namespace str, 2:peer_id bytes, 3:addrs rep,
               4:ttl_secs uint32, 5:issued_at uint64}
Encoding follows protobuf standard: default values are omitted (bool false,
empty bytes/string, zero integers); fields sorted by tag; unknown fields on
decode are collected but ignored (forward compatibility, spec section 6).
"""

from typing import Dict, List, Optional, Tuple

from framing import FrameError, decode_varint, encode_varint

WT_VARINT = 0
WT_LEN = 2


def _tag(field: int, wire_type: int) -> bytes:
    return encode_varint((field << 3) | wire_type)


def _enc_str(field: int, value: str) -> bytes:
    if not value:
        return b""
    raw = value.encode("utf-8")
    return _tag(field, WT_LEN) + encode_varint(len(raw)) + raw


def _enc_bytes(field: int, value: bytes) -> bytes:
    if not value:
        return b""
    return _tag(field, WT_LEN) + encode_varint(len(value)) + value


def _enc_uint(field: int, value: int) -> bytes:
    if value == 0:
        return b""
    return _tag(field, WT_VARINT) + encode_varint(value)


def _enc_bool(field: int, value: bool) -> bytes:
    if not value:
        return b""
    return _tag(field, WT_VARINT) + b"\x01"


def _enc_msg(field: int, raw: bytes) -> bytes:
    return _tag(field, WT_LEN) + encode_varint(len(raw)) + raw


def encode_addrmsg(quic: bool, ip: str, port: int) -> bytes:
    return _enc_bool(1, quic) + _enc_str(2, ip) + _enc_uint(3, port)


def encode_signed_fields(
    namespace: str,
    peer_id: bytes,
    addrs: List[bytes],
    ttl_secs: int,
    issued_at: int,
) -> bytes:
    out = _enc_str(1, namespace) + _enc_bytes(2, peer_id)
    for addr in addrs:
        out += _enc_msg(3, addr)
    out += _enc_uint(4, ttl_secs) + _enc_uint(5, issued_at)
    return out


def encode_register(
    namespace: str,
    peer_id: bytes,
    pubkey: bytes,
    addrs: List[bytes],
    ttl_secs: int,
    sig: bytes,
    issued_at: int,
) -> bytes:
    out = _enc_str(1, namespace) + _enc_bytes(2, peer_id) + _enc_bytes(3, pubkey)
    for addr in addrs:
        out += _enc_msg(4, addr)
    out += _enc_uint(5, ttl_secs) + _enc_bytes(6, sig) + _enc_uint(7, issued_at)
    return out


def encode_query(namespace: str, peer_id: Optional[bytes] = None) -> bytes:
    out = _enc_str(1, namespace)
    if peer_id:
        out += _enc_bytes(2, peer_id)
    return out


def encode_request_register(raw: bytes) -> bytes:
    return _enc_msg(1, raw)


def encode_request_query(raw: bytes) -> bytes:
    return _enc_msg(2, raw)


class PbDecodeError(FrameError):
    pass


def _iter_fields(data: bytes):
    pos = 0
    while pos < len(data):
        tag, pos = decode_varint(data, pos)
        field, wire_type = tag >> 3, tag & 7
        if wire_type == WT_VARINT:
            value, pos = decode_varint(data, pos)
        elif wire_type == WT_LEN:
            length, pos = decode_varint(data, pos)
            if pos + length > len(data):
                raise PbDecodeError("length-delimited field overruns message")
            value = data[pos : pos + length]
            pos += length
        else:
            raise PbDecodeError("unsupported wire type %d" % wire_type)
        yield field, value


class AddrMsg:
    def __init__(self, quic: bool, ip: str, port: int):
        self.quic = quic
        self.ip = ip
        self.port = port

    def __repr__(self) -> str:
        return "AddrMsg(%s %s:%d)" % ("u" if self.quic else "t", self.ip, self.port)


def decode_addrmsg(data: bytes) -> AddrMsg:
    quic, ip, port = False, "", 0
    for field, value in _iter_fields(data):
        if field == 1:
            quic = bool(value)
        elif field == 2:
            ip = value.decode("utf-8")
        elif field == 3:
            port = value
    if port > 65535:
        raise PbDecodeError("addr port out of range: %d" % port)
    return AddrMsg(quic, ip, port)


class PeerEntry:
    def __init__(self, peer_id: bytes, addrs: List[AddrMsg]):
        self.peer_id = peer_id
        self.addrs = addrs


class Response:
    def __init__(self, error: str, peers: List[PeerEntry]):
        self.error = error
        self.peers = peers


def decode_response(data: bytes) -> Response:
    error, peers = "", []
    for field, value in _iter_fields(data):
        if field == 1:
            error = value.decode("utf-8")
        elif field == 2:
            peer_id, addrs = b"", []
            for sub, subval in _iter_fields(value):
                if sub == 1:
                    peer_id = subval
                elif sub == 2:
                    addrs.append(decode_addrmsg(subval))
            peers.append(PeerEntry(peer_id, addrs))
    return Response(error, peers)
