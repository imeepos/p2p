"""Frame codec: unsigned LEB128 varint length prefix + payload.

Docs: docs/protocol/wire-format.md section 6 and vectors/varint.json.
- MAX_FRAME_SIZE = 1048576 (1 MiB); oversized length on write raises before
  any byte is written; on read aborts the stream before reading payload.
- varint: max 10 bytes, 10th byte must carry no continuation bit and its
  data bits must be <= 1 (no silent 64-bit wraparound).
"""

import asyncio
from typing import Optional

MAX_FRAME_SIZE = 1048576
MAX_VARINT_LEN = 10


class FrameError(Exception):
    """Protocol-level framing failure; caller must abort the stream."""


class FrameTooLarge(FrameError):
    pass


class VarintOverflow(FrameError):
    pass


class UnexpectedEof(FrameError):
    pass


def encode_varint(value: int) -> bytes:
    if value < 0:
        raise FrameError("varint is unsigned, got %d" % value)
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def decode_varint(data: bytes, offset: int = 0) -> "tuple[int, int]":
    """Decode one varint from data; returns (value, next_offset).
    Raises VarintOverflow on the documented 10th-byte conditions."""
    result = 0
    shift = 0
    pos = offset
    while True:
        if pos >= len(data):
            raise FrameError("varint truncated")
        byte = data[pos]
        pos += 1
        if shift == 63:  # 10th byte
            if byte & 0x80:
                raise VarintOverflow("varint 10th byte continues")
            if byte > 0x01:
                raise VarintOverflow("varint 10th byte data bit > 1")
        result |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return result, pos
        shift += 7
        if shift > 63:
            raise VarintOverflow("varint exceeds 10 bytes")


def encode_frame(payload: bytes) -> bytes:
    if len(payload) > MAX_FRAME_SIZE:
        raise FrameTooLarge(
            "frame payload %d exceeds MAX_FRAME_SIZE %d" % (len(payload), MAX_FRAME_SIZE)
        )
    return encode_varint(len(payload)) + payload


async def read_frame(reader: asyncio.StreamReader) -> Optional[bytes]:
    """Read one frame; returns None on clean EOF at a frame boundary."""
    prefix = bytearray()
    while len(prefix) < MAX_VARINT_LEN:
        try:
            byte = await reader.readexactly(1)
        except asyncio.IncompleteReadError as exc:
            if not prefix and not exc.partial:
                return None
            raise UnexpectedEof("eof inside frame length prefix")
        prefix.append(byte[0])
        if not byte[0] & 0x80:
            break
    else:
        raise VarintOverflow("varint length prefix exceeds 10 bytes")
    length, consumed = decode_varint(bytes(prefix))
    if consumed != len(prefix):
        raise FrameError("internal varint length mismatch")
    if length > MAX_FRAME_SIZE:
        raise FrameTooLarge(
            "declared frame length %d exceeds MAX_FRAME_SIZE" % length
        )
    try:
        return await reader.readexactly(length)
    except asyncio.IncompleteReadError as exc:
        raise UnexpectedEof(
            "declared %d bytes but got %d before eof" % (length, len(exc.partial))
        )


async def write_frame(writer: asyncio.StreamWriter, payload: bytes) -> None:
    writer.write(encode_frame(payload))
    await writer.drain()


async def write_raw(writer: asyncio.StreamWriter, data: bytes) -> None:
    """Write pre-framed bytes (used by protocols with their own link framing)."""
    writer.write(data)
    await writer.drain()


async def read_exact_or_eof(
    reader: asyncio.StreamReader, count: int
) -> Optional[bytes]:
    """Read exactly count bytes; None on clean EOF with nothing buffered."""
    try:
        return await reader.readexactly(count)
    except asyncio.IncompleteReadError as exc:
        if not exc.partial:
            return None
        raise UnexpectedEof(
            "wanted %d bytes, got %d before eof" % (count, len(exc.partial))
        )
