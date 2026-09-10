"""QUIC transport for mininode: identity certs, ALPN, mutual PeerId anchoring.

Docs: docs/protocol/wire-format.md sections 4.2/4.4, 5, 8.
- TLS1.3 only, ALPN fixed "p2p-base/1", SNI placeholder "p2p-base".
- Both sides present self-signed identity certs; the peer certificate must
  carry the private identity extension and SPKI==extension (certmgr).
- Remote PeerId is derived from the cert, never from self-reported strings;
  dialing with an expected PeerId mismatch aborts the connection before any
  application data is exchanged (PeerMismatch semantics, wire-format 4.4).
aioquic notes: CA-based verification is switched off (CERT_NONE) because the
protocol replaces it with extension-based identity; the identity assertion
runs right after the handshake. Raising a TLS alert locally instead would
stall aioquic's wait_connected (observed on 1.2.0), so the documented
"abort" is implemented as an immediate close + explicit error.
"""

import asyncio
import ssl
from contextlib import AsyncExitStack

from aioquic.asyncio.client import connect as quic_connect
from aioquic.asyncio.protocol import QuicConnectionProtocol
from aioquic.asyncio.server import serve as quic_serve
from aioquic.quic.configuration import QuicConfiguration

from certmgr import IdentityCertError, build_identity_certificate, verify_identity_certificate
from identity import Identity

ALPN = "p2p-base/1"
SNI_PLACEHOLDER = "p2p-base"


def build_configuration(ident: Identity, is_client: bool) -> QuicConfiguration:
    config = QuicConfiguration(
        is_client=is_client,
        alpn_protocols=[ALPN],
        server_name=SNI_PLACEHOLDER if is_client else None,
        verify_mode=ssl.CERT_NONE,
    )
    config.certificate = build_identity_certificate(ident)
    config.private_key = ident.private_key
    return config


def remote_peer_id_of(protocol: QuicConnectionProtocol):
    """Derive the remote PeerId from the negotiated TLS peer certificate."""
    cert = getattr(protocol._quic.tls, "_peer_certificate", None)
    if cert is None:
        return None
    try:
        return verify_identity_certificate(cert)
    except IdentityCertError:
        return None


def assert_remote_peer_id(protocol: QuicConnectionProtocol, expected: str) -> str:
    """Enforce identity verification and expected PeerId; raises otherwise."""
    cert = getattr(protocol._quic.tls, "_peer_certificate", None)
    if cert is None:
        raise IdentityCertError("remote endpoint presented no certificate")
    derived = verify_identity_certificate(cert)
    if expected and derived != expected:
        raise IdentityCertError(
            "peer mismatch: expected %s, got %s" % (expected, derived)
        )
    return derived


class DialHandle:
    """Owns the aioquic connect() context-manager lifecycle."""

    def __init__(self, stack: AsyncExitStack, protocol: QuicConnectionProtocol):
        self._stack = stack
        self.protocol = protocol

    async def close(self) -> None:
        await self._stack.aclose()


async def dial(
    host: str,
    port: int,
    ident: Identity,
    expected_peer_id: str,
    local_port: int = 0,
) -> DialHandle:
    """Open a QUIC connection with mutual identity verification."""
    stack = AsyncExitStack()
    try:
        protocol = await stack.enter_async_context(
            quic_connect(
                host,
                port,
                configuration=build_configuration(ident, is_client=True),
                local_port=local_port,
            )
        )
        assert_remote_peer_id(protocol, expected_peer_id)
    except BaseException:
        await stack.aclose()
        raise
    return DialHandle(stack, protocol)


class QuicServerHandle:
    def __init__(self, server, ident: Identity):
        self._server = server
        self.ident = ident

    async def wait_closed(self) -> None:
        await self._server.wait_closed()

    def close(self) -> None:
        self._server.close()


async def listen(
    host: str,
    port: int,
    ident: Identity,
    stream_handler,
) -> QuicServerHandle:
    """Serve QUIC; stream_handler is an async callable(reader, writer)."""

    def spawn(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        asyncio.get_event_loop().create_task(stream_handler(reader, writer))

    server = await quic_serve(
        host,
        port,
        configuration=build_configuration(ident, is_client=False),
        stream_handler=spawn,
    )
    return QuicServerHandle(server, ident)
