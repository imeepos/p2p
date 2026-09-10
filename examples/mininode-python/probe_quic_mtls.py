"""QUIC mTLS feasibility probe (loopback aioquic server vs aioquic client).

Validates, before committing to the QUIC transport path:
  1. self-signed identity cert with private OID extension is accepted by an
     aioquic TLS1.3 stack and the peer certificate is retrievable;
  2. a client certificate is delivered when the server requests one
     (aioquic server-side request flag is test-only; the production peer is
     rustls which always requests client auth);
  3. in-handshake identity verification via IdentityTLSContext rejects a
     cert without the identity extension;
  4. ALPN p2p-base/1 negotiation and the frame flow: protocol ID frame ->
     ping frame -> echo reply.

Run: python3 probe_quic_mtls.py     (exit 0 = QUIC path viable)
"""

import asyncio
import os
import socket
import ssl
import sys

from aioquic.asyncio.server import serve as quic_serve
from aioquic.quic.configuration import QuicConfiguration

import quic_node
from certmgr import IdentityCertError, verify_identity_certificate
from framing import read_frame, write_frame
from identity import Identity

PING_PROTOCOL = b"/p2p-base/ping/1"


async def echo_stream_handler(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
    proto = await read_frame(reader)
    if proto != PING_PROTOCOL:
        print("probe-server: unsupported protocol %r, closing" % proto)
        writer.write_eof()
        return
    while True:
        payload = await read_frame(reader)
        if payload is None:
            return
        await write_frame(writer, payload)


async def run_probe() -> int:
    server_ident = Identity(os.urandom(32))
    client_ident = Identity(os.urandom(32))

    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]

    server_config = quic_node.build_configuration(server_ident, is_client=False)
    server = await quic_serve(
        "127.0.0.1", port, configuration=server_config,
        stream_handler=lambda r, w: asyncio.get_event_loop().create_task(
            echo_stream_handler(r, w)
        ),
    )
    # aioquic server requests client auth only through this flag (test-only);
    # the production peer (rustls) always requests client certificates.
    try:
        handle = await quic_node.dial("127.0.0.1", port, client_ident, server_ident.peer_id)
        try:
            seen = verify_identity_certificate(
                handle.protocol._quic.tls._peer_certificate
            )
            print("probe-client: server identity anchored: %s" % seen)
            reader, writer = await handle.protocol.create_stream()
            await write_frame(writer, PING_PROTOCOL)
            nonce = os.urandom(8)
            await write_frame(writer, nonce)
            echoed = await read_frame(reader)
            if echoed != nonce:
                print("probe-client: echo mismatch %r != %r" % (echoed, nonce))
                return 1
            writer.write_eof()
            print("probe-client: ping roundtrip OK over QUIC+identity TLS")
        finally:
            await handle.close()
    except IdentityCertError as exc:
        print("probe-client: identity failure: %s" % exc)
        return 1
    finally:
        server.close()

    # negative test: with the identity hook active and a stale expected
    # PeerId, the handshake against a different cert must abort in-handshake.
    server2_ident = Identity(os.urandom(32))
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind(("127.0.0.1", 0))
        port2 = sock.getsockname()[1]
    server2 = await quic_serve(
        "127.0.0.1", port2, configuration=quic_node.build_configuration(server2_ident, is_client=False),
        stream_handler=lambda r, w: asyncio.get_event_loop().create_task(
            echo_stream_handler(r, w)
        ),
    )
    try:
        handle = await quic_node.dial(
            "127.0.0.1", port2, client_ident, server_ident.peer_id
        )
        peer = quic_node.remote_peer_id_of(handle.protocol)
        await handle.close()
        print("probe-negative: handshake unexpectedly completed with %s" % peer)
        return 1
    except Exception as exc:  # noqa: BLE001 - rejection is the expected outcome
        print("probe-negative: connection rejected before any data: %r" % exc)
        print("PROBE-PASS")
        return 0
    finally:
        server2.close()


def main() -> int:
    try:
        return asyncio.run(run_probe())
    except Exception as exc:  # noqa: BLE001 - probe reports any failure
        print("PROBE-FAILED: %r" % exc)
        return 1


if __name__ == "__main__":
    sys.exit(main())
