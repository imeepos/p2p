"""/p2p-base/ping/1 initiator and responder.

Docs: docs/protocol/specs/ping.md.
Initiator: open stream -> protocol ID frame -> 1 request frame -> read 1 echo
frame -> byte-compare -> close. Responder: read 1 frame, echo it, return.
Nonce content is arbitrary; the protocol gives it no semantics (spec 2/5).
"""

import os
import time
from typing import Tuple

from framing import read_frame, write_frame

PING_PROTOCOL = b"/p2p-base/ping/1"


async def ping_once(reader_writer) -> Tuple[bytes, float]:
    """Run one ping roundtrip; returns (nonce, rtt_seconds).
    Raises FrameError/UnexpectedEof on framing or transport failure."""
    reader, writer = reader_writer
    nonce = os.urandom(8)
    started = time.monotonic()
    await write_frame(writer, PING_PROTOCOL)
    await write_frame(writer, nonce)
    echoed = await read_frame(reader)
    elapsed = time.monotonic() - started
    if echoed is None:
        raise ConnectionError("ping: stream closed before pong (unsupported?)")
    if echoed != nonce:
        raise ConnectionError(
            "ping: echo mismatch: %d bytes != %d bytes" % (len(echoed), len(nonce))
        )
    # server closes its side as soon as it answered; no write_eof needed
    return nonce, elapsed


async def serve_ping_stream(reader_writer) -> None:
    """Answer one ping: read the first frame after the protocol ID, echo it."""
    reader, writer = reader_writer
    payload = await read_frame(reader)
    if payload is None:
        return
    await write_frame(writer, payload)
