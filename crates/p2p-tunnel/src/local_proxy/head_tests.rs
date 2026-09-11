//! Head 单测（迁移自 apps/gui/src-tauri/src/tunnel/head.rs 内联 tests，逐字）。

use super::*;

#[test]
fn parses_and_rewrites_host() {
    let raw = b"GET /x HTTP/1.1\r\nHost: 127.0.0.1:40001\r\nAccept: */*\r\n\r\nbody!";
    let pos = find_head_end(raw).expect("head end");
    let mut head = Head::parse(&raw[..pos]).expect("parse");
    assert_eq!(head.first_line, "GET /x HTTP/1.1");
    assert_eq!(head.header("host"), Some("127.0.0.1:40001"));
    head.rewrite_host(3080);
    assert_eq!(head.header("Host"), Some("127.0.0.1:3080"));
    assert_eq!(head.header("Accept"), Some("*/*"));
    assert_eq!(head.content_length().unwrap(), 0);
    assert!(!head.is_websocket_upgrade());
}

#[test]
fn origin_referer_rewrite_to_target_authority() {
    let raw = "POST /api/x HTTP/1.1\r\nHost: 127.0.0.1:40001\r\n\
               Origin: http://127.0.0.1:52172\r\n\
               Referer: http://127.0.0.1:52172/?token=t\r\n\r\n";
    let mut head = Head::parse(raw.as_bytes()).expect("parse");
    head.rewrite_host(3080);
    assert_eq!(head.header("Origin"), Some("http://127.0.0.1:3080"));
    assert_eq!(
        head.header("Referer"),
        Some("http://127.0.0.1:3080/?token=t")
    );
}

#[test]
fn ws_upgrade_origin_rewritten_too() {
    let raw = "GET /api/remote.mux HTTP/1.1\r\nHost: a\r\nUpgrade: WebSocket\r\n\
               Connection: Upgrade\r\nOrigin: http://127.0.0.1:52172\r\n\r\n";
    let mut head = Head::parse(raw.as_bytes()).expect("parse");
    assert!(head.is_websocket_upgrade());
    head.rewrite_host(3080);
    assert_eq!(head.header("Origin"), Some("http://127.0.0.1:3080"));
}

#[test]
fn origin_referer_left_alone_when_absent_or_foreign() {
    let raw = "GET / HTTP/1.1\r\nHost: h\r\nOrigin: null\r\n\
               Referer: http://localhost:9/p\r\n\r\n";
    let mut head = Head::parse(raw.as_bytes()).expect("parse");
    head.rewrite_host(3080);
    assert_eq!(head.header("Origin"), Some("null"));
    assert_eq!(head.header("Referer"), Some("http://localhost:9/p"));
    let mut bare = Head::parse(b"GET / HTTP/1.1\r\nHost: h\r\n\r\n").expect("parse");
    bare.rewrite_host(3080);
    assert!(bare.header("Origin").is_none());
    assert!(bare.header("Referer").is_none());
}

#[test]
fn websocket_detection_and_hop_by_hop() {
    let raw = "GET /api/remote.mux HTTP/1.1\r\nHost: a\r\nUpgrade: WebSocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: x\r\n\r\n";
    let mut head = Head::parse(raw.as_bytes()).expect("parse");
    assert!(head.is_websocket_upgrade());
    head.apply_hop_by_hop(true);
    assert_eq!(head.header("Connection"), Some("Upgrade"));
    head.apply_hop_by_hop(false);
    assert_eq!(head.header("Connection"), Some("close"));
    assert!(head.header("Keep-Alive").is_none());
}

#[test]
fn wire_round_trip_and_leftover_split() {
    let raw = b"POST /p HTTP/1.1\r\nHost: h\r\nContent-Length: 5\r\n\r\nhello";
    let pos = find_head_end(raw).expect("head end");
    let head = Head::parse(&raw[..pos]).expect("parse");
    assert_eq!(head.content_length().unwrap(), 5);
    assert_eq!(&raw[pos + 4..], b"hello");
    let wire = head.to_wire();
    assert!(wire.ends_with(b"\r\n\r\n"));
    assert!(std::str::from_utf8(&wire)
        .unwrap()
        .starts_with("POST /p HTTP/1.1\r\nHost: h\r\nContent-Length: 5\r\n\r\n"));
}

#[test]
fn chunked_flag_and_bad_length() {
    let mut head = Head::parse(b"POST / HTTP/1.1\r\nHost: h\r\nTransfer-Encoding: chunked\r\n\r\n")
        .expect("parse");
    assert!(head.has_chunked_body());
    head.set_header("Content-Length", "abc");
    assert!(head.content_length().is_err());
}
