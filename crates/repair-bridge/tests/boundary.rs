use repair_bridge::{pump, stdin_to_stream, stream_to_stdout, IO_CHUNK_SIZE};
use std::process::Command;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

fn frame_bytes(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut len = payload.len() as u64;
    loop {
        let mut byte = (len & 0x7f) as u8;
        len >>= 7;
        if len != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if len == 0 {
            break;
        }
    }
    out.extend_from_slice(payload);
    out
}

#[tokio::test]
async fn empty_payload_frame_is_transferred() {
    let (mut input_w, input_r) = duplex(64);
    let (output_w, mut output_r) = duplex(64);
    p2p_protocol::write_frame(&mut input_w, &[]).await.unwrap();
    drop(input_w);
    let task = tokio::spawn(stream_to_stdout(input_r, output_w));
    let mut got = Vec::new();
    output_r.read_to_end(&mut got).await.unwrap();
    assert!(got.is_empty());
    assert!(task.await.unwrap().is_err());
}

#[tokio::test]
async fn exact_one_mib_passes_and_oversize_is_rejected() {
    let payload = vec![0x5a; IO_CHUNK_SIZE];
    let (mut source_w, source_r) = duplex(IO_CHUNK_SIZE + 64);
    let (stream_w, mut stream_r) = duplex(IO_CHUNK_SIZE + 64);
    let expected = payload.clone();
    let writer = tokio::spawn(async move { source_w.write_all(&expected).await });
    stdin_to_stream(source_r, stream_w).await.unwrap();
    writer.await.unwrap().unwrap();
    assert_eq!(
        p2p_protocol::read_frame(&mut stream_r).await.unwrap(),
        payload
    );
    let oversize = frame_bytes(&vec![1u8; IO_CHUNK_SIZE + 1]);
    let mut reader = &oversize[..];
    let error = p2p_protocol::read_frame(&mut reader)
        .await
        .expect_err("oversize must fail");
    assert!(error.to_string().contains("frame too large"));
}

#[tokio::test]
async fn split_frame_arrival_reassembles_correctly() {
    let payload = b"split-frame-payload";
    let bytes = frame_bytes(payload);
    let (mut writer, reader) = duplex(64);
    let send = tokio::spawn(async move {
        for part in bytes.chunks(2) {
            writer.write_all(part).await.unwrap();
        }
    });
    let mut reader = reader;
    assert_eq!(
        p2p_protocol::read_frame(&mut reader).await.unwrap(),
        payload
    );
    send.await.unwrap();
}

#[tokio::test]
async fn sticky_frames_in_one_write_are_read_in_order() {
    let first = b"first";
    let second = b"second";
    let mut bytes = frame_bytes(first);
    bytes.extend(frame_bytes(second));
    let (mut writer, mut reader) = duplex(64);
    writer.write_all(&bytes).await.unwrap();
    assert_eq!(p2p_protocol::read_frame(&mut reader).await.unwrap(), first);
    assert_eq!(p2p_protocol::read_frame(&mut reader).await.unwrap(), second);
}

#[tokio::test]
async fn binary_stdin_bytes_are_preserved() {
    let payload = vec![0, 0xff, 0x80, 1, 2, 0];
    let (mut source_w, source_r) = duplex(64);
    let (stream_w, mut stream_r) = duplex(64);
    let expected = payload.clone();
    let writer = tokio::spawn(async move {
        source_w.write_all(&expected).await.unwrap();
    });
    stdin_to_stream(source_r, stream_w).await.unwrap();
    writer.await.unwrap();
    assert_eq!(
        p2p_protocol::read_frame(&mut stream_r).await.unwrap(),
        payload
    );
}

#[tokio::test]
async fn simultaneous_small_samples_stay_on_their_streams() {
    let (left_w, mut left_r) = duplex(64);
    let (mut right_w, right_r) = duplex(64);
    let (mut input_w, input_r) = duplex(64);
    let (output_w, mut output_r) = duplex(64);
    let left = tokio::spawn(async move { stdin_to_stream(input_r, left_w).await });
    let right = tokio::spawn(async move { stream_to_stdout(right_r, output_w).await });
    input_w.write_all(b"left").await.unwrap();
    drop(input_w);
    p2p_protocol::write_frame(&mut right_w, b"right")
        .await
        .unwrap();
    drop(right_w);
    assert_eq!(
        p2p_protocol::read_frame(&mut left_r).await.unwrap(),
        b"left"
    );
    let mut output = Vec::new();
    output_r.read_to_end(&mut output).await.unwrap();
    left.await.unwrap().unwrap();
    assert!(right.await.unwrap().is_err());
    assert_eq!(output, b"right");
}

#[tokio::test]
async fn stdin_immediate_eof_is_nonzero_with_reason() {
    let (stdin_w, stdin_r) = duplex(32);
    drop(stdin_w);
    let (stream_w, _stream_r) = duplex(32);
    let (peer_w, peer_r) = duplex(32);
    drop(peer_w);
    let (stdout_w, _stdout_r) = duplex(32);
    let error = pump(stdin_r, stream_w, peer_r, stdout_w)
        .await
        .expect_err("EOF must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    assert!(!error.to_string().is_empty());
}

#[test]
fn unknown_peer_cli_exits_nonzero_with_stderr_reason() {
    let output = Command::new(env!("CARGO_BIN_EXE_repair-bridge"))
        .args([
            "--ticket",
            "opaque",
            "--peer",
            "invalid-peer",
            "--bootstrap",
            "127.0.0.1/u1",
        ])
        .output()
        .expect("bridge binary must run");
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn missing_peer_and_bootstrap_are_rejected() {
    let peer_missing = Command::new(env!("CARGO_BIN_EXE_repair-bridge"))
        .args(["--ticket", "opaque", "--bootstrap", "127.0.0.1/u1"])
        .output()
        .unwrap();
    let bootstrap_missing = Command::new(env!("CARGO_BIN_EXE_repair-bridge"))
        .args(["--ticket", "opaque", "--peer", "invalid-peer"])
        .output()
        .unwrap();
    assert_eq!(peer_missing.status.code(), Some(2));
    assert_eq!(bootstrap_missing.status.code(), Some(2));
    assert!(!peer_missing.stderr.is_empty());
    assert!(!bootstrap_missing.stderr.is_empty());
}
