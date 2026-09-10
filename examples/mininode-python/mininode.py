#!/usr/bin/env python3
"""mininode: a third-party minimal p2p-base node implemented from
docs/protocol/ only (Python 3.9, aioquic transport).

Typical run against a local bootstrap node:

    python3 mininode.py --seed ./seed.bin --bootstrap 127.0.0.1/u3400 \
        --peer <bootstrap-peer-id> --namespace interop-demo --listen 127.0.0.1:0

Steps: connect with mutual identity verification, register our address,
query the namespace to confirm visibility, ping the bootstrap node.
Each enabled step prints an OK marker; any failure exits non-zero.
"""

import argparse
import asyncio
import sys
import time
from typing import List, Optional

import pbcodec as pb
import ping as ping_proto
import rendezvous as rz
import quic_node
from identity import Identity, load_identity

QUIT = 1


def log(message: str) -> None:
    print("[%s] %s" % (time.strftime("%H:%M:%S"), message), file=sys.stderr)


def parse_transport_addr(text: str, default_host: str = "127.0.0.1"):
    """Accept ip/uPORT (QUIC) or ip:PORT; returns (host, port)."""
    if "/u" in text:
        host, port = text.split("/u", 1)
    elif "/t" in text:
        host, port = text.split("/t", 1)
    elif ":" in text:
        host, port = text.rsplit(":", 1)
    else:
        host, port = default_host, text
    return host or default_host, int(port)


async def run_listen(ident: Identity, listen: str):
    host, port = parse_transport_addr(listen)
    handle = await quic_node.listen(host, port, ident, ping_proto.serve_ping_stream)
    log("listening on %s:%d (peer %s)" % (host, port, ident.peer_id))
    return handle


def advertised_addrs(args, listen_handle) -> List[pb.AddrMsg]:
    if args.advertise:
        host, port = parse_transport_addr(args.advertise)
        return [pb.AddrMsg(True, host, port)]
    if listen_handle is None:
        return []
    sock = listen_handle._server._transport.get_extra_info("sockname")
    return [pb.AddrMsg(True, sock[0], sock[1])]


async def step_register(handle, ident, args, addrs) -> None:
    writer_conn = await handle.protocol.create_stream()
    response = await rz.register(
        writer_conn, ident, args.namespace, addrs, ttl_secs=args.ttl_secs
    )
    rz.expect_success(response, "register")
    print("REGISTER-OK namespace=%s addrs=%d ttl=%ds"
          % (args.namespace, len(addrs), args.ttl_secs))


async def step_query(handle, ident, args) -> None:
    conn = await handle.protocol.create_stream()
    import hashlib
    from identity import base58_decode

    exact = base58_decode(args.query_peer) if args.query_peer else None
    response = await rz.query(conn, args.namespace, exact)
    rz.expect_success(response, "query")
    summary = ["%s@%s" % (
        peer.peer_id.hex()[:8],
        ",".join("%s:%d" % (a.ip, a.port) for a in peer.addrs) or "-",
    ) for peer in response.peers]
    if exact is None:
        own = hashlib.sha256(ident.pubkey).digest()
        if not any(peer.peer_id == own for peer in response.peers):
            raise rz.RendezvousError(
                "query: own entry missing after register (entries=%d)" % len(response.peers)
            )
    print("QUERY-OK namespace=%s entries=%d %s"
          % (args.namespace, len(response.peers), " ".join(summary)))


async def step_ping(handle, args) -> None:
    conn = await handle.protocol.create_stream()
    _, elapsed = await ping_proto.ping_once(conn)
    print("PING-OK peer=%s rtt_ms=%.1f" % (args.peer, elapsed * 1000))


async def run(args) -> int:
    ident, _seed = load_identity(args.seed)
    log("identity peer=%s" % ident.peer_id)
    if args.peer and args.peer == ident.peer_id:
        print("refusing to dial self (%s)" % ident.peer_id, file=sys.stderr)
        return QUIT

    listen_handle = None
    if args.listen:
        listen_handle = await run_listen(ident, args.listen)

    handle = None
    try:
        host, port = parse_transport_addr(args.bootstrap)
        handle = await quic_node.dial(
            host, port, ident, args.peer, local_port=args.local_port
        )
        log("connected to bootstrap %s:%d peer=%s" % (host, port, args.peer))
        addrs = advertised_addrs(args, listen_handle)

        if args.only in ("register", "all"):
            await step_register(handle, ident, args, addrs)
        if args.only in ("query", "all"):
            await step_query(handle, ident, args)
        if args.only in ("ping", "all"):
            if not args.peer:
                print("--ping requires --peer", file=sys.stderr)
                return QUIT
            await step_ping(handle, args)
    finally:
        if handle:
            await handle.close()
        if listen_handle:
            listen_handle.close()
    print("MININODE-OK peer=%s" % ident.peer_id)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="mininode.py",
        description="Minimal p2p-base node (QUIC/aioquic) built from docs only.",
    )
    parser.add_argument("--seed", required=True, help="Ed25519 seed file (32B, 0600)")
    parser.add_argument("--listen", default=None,
                        help="local QUIC listen address host:port (answers ping)")
    parser.add_argument("--bootstrap", default=None,
                        help="rendezvous/bootstrap address ip/uPORT or ip:PORT")
    parser.add_argument("--peer", default=None,
                        help="expected base58 PeerId of the bootstrap node")
    parser.add_argument("--namespace", default="mininode-demo",
                        help="rendezvous namespace to register/query")
    parser.add_argument("--ttl-secs", type=int, default=rz.DEFAULT_TTL_SECS,
                        help="registration TTL seconds (default 60)")
    parser.add_argument("--advertise", default=None,
                        help="address to register ip:port (default: listen addr)")
    parser.add_argument("--query-peer", default=None,
                        help="exact base58 PeerId to query (default: own peer)")
    parser.add_argument("--local-port", type=int, default=0,
                        help="local UDP port for the client socket")
    parser.add_argument("--only", choices=["register", "query", "ping", "all"],
                        default="all", help="run a single step (default all)")
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    if not args.bootstrap:
        print("--bootstrap is required", file=sys.stderr)
        return 2
    try:
        return asyncio.run(asyncio.wait_for(run(args), timeout=60))
    except asyncio.TimeoutError:
        print("mininode: run timed out", file=sys.stderr)
        return QUIT
    except KeyboardInterrupt:
        return 130
    except Exception as exc:  # noqa: BLE001 - CLI reports failures explicitly
        print("mininode: %s: %s" % (type(exc).__name__, exc), file=sys.stderr)
        return QUIT


if __name__ == "__main__":
    sys.exit(main())
