use super::*;
use tokio::io::duplex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn xx_handshake_binds_peer_identity() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let (a, b) = duplex(64 * 1024);
    let sa: BoxedStream = Box::new(a);
    let sb: BoxedStream = Box::new(b);

    let server = NoiseXx::new();
    let client = NoiseXx::new();
    let (server, client) = tokio::join!(
        server.inbound(sb, &bob),
        client.outbound(sa, &alice, Some(bob.peer_id()))
    );
    let (server_peer, mut server_stream) = server.expect("server handshake");
    let (client_peer, mut client_stream) = client.expect("client handshake");
    assert_eq!(server_peer, alice.peer_id(), "server sees client identity");
    assert_eq!(client_peer, bob.peer_id(), "client sees server identity");

    client_stream.write_all(b"secret").await.expect("write");
    client_stream.flush().await.expect("flush");
    let mut buf = [0u8; 6];
    server_stream.read_exact(&mut buf).await.expect("read");
    assert_eq!(&buf, b"secret");
}

#[tokio::test]
async fn wrong_expected_peer_is_rejected() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let eve = Keypair::generate();
    let (a, b) = duplex(64 * 1024);
    let sa: BoxedStream = Box::new(a);
    let sb: BoxedStream = Box::new(b);

    let server = NoiseXx::new();
    let client = NoiseXx::new();
    let (server, client) = tokio::join!(
        server.inbound(sb, &bob),
        client.outbound(sa, &alice, Some(eve.peer_id()))
    );
    assert!(
        server.is_err() || client.is_err(),
        "expected mismatch must fail"
    );
}
