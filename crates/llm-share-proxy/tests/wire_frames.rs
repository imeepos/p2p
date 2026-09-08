//! /llm-share/proxy/1 线格式回归（分发支路 FRAME_SINGLE、协议 ID 支路、
//! 信封/扁平双形态解析与上游体剥离）。断言经由公开接缝 read_request_frame /
//! ProxyRequest，见 src/wire.rs。

use llm_share_proxy::wire::read_request_frame;
use llm_share_proxy::{ProxyFrame, ProxyRequest, PROTOCOL_ID};
use p2p_mux::BoxedStream;
use p2p_protocol::{
    write_chunked, write_frame, write_protocol_id, ProtocolId, FRAME_CHUNK, FRAME_END, FRAME_SINGLE,
};
use serde_json::Value;
use tokio::io::AsyncWriteExt;

fn sample() -> Value {
    serde_json::json!({
        "req_id": "r-1", "model": "gpt-4o", "max_tokens": 128,
        "messages": [{ "role": "user", "content": "ping" }], "stream": true
    })
}

#[test]
fn parse_extracts_gates_fields() {
    let raw = serde_json::to_vec(&sample()).expect("json");
    let req = ProxyRequest::parse(&raw).expect("valid");
    assert_eq!(req.req_id, "r-1");
    assert_eq!(req.model, "gpt-4o");
    assert_eq!(req.max_tokens, 128);
    assert_eq!(req.wire_bytes, raw.len());
}

#[test]
fn upstream_body_strips_proxy_field() {
    let raw = serde_json::to_vec(&sample()).expect("json");
    let req = ProxyRequest::parse(&raw).expect("valid");
    assert!(req.body.get("req_id").is_some());
    assert!(req.upstream_body().get("req_id").is_none());
    assert_eq!(req.upstream_body()["model"], "gpt-4o");
}

#[test]
fn missing_required_fields_rejected() {
    let mut no_tokens = sample();
    no_tokens.as_object_mut().expect("obj").remove("max_tokens");
    assert!(ProxyRequest::parse(&serde_json::to_vec(&no_tokens).expect("json")).is_err());
    let mut no_id = sample();
    no_id.as_object_mut().expect("obj").remove("req_id");
    assert!(ProxyRequest::parse(&serde_json::to_vec(&no_id).expect("json")).is_err());
    assert!(ProxyRequest::parse(b"not-json").is_err());
}

/// 信封形态（ProxyClient 序列化全结构）：解析取内层 body，
/// upstream_body 得到纯 OpenAI 体且无任何 req_id 残留。
#[test]
fn parse_envelope_extracts_inner_body() {
    let envelope = serde_json::json!({
        "req_id": "r-2",
        "body": {
            "req_id": "r-2",
            "model": "gpt-4o",
            "max_tokens": 64,
            "messages": [{ "role": "user", "content": "ping" }],
            "stream": true
        },
        "model": "gpt-4o",
        "max_tokens": 64,
        "wire_bytes": 0
    });
    let raw = serde_json::to_vec(&envelope).expect("json");
    let req = ProxyRequest::parse(&raw).expect("valid envelope");
    assert_eq!(req.req_id, "r-2");
    assert!(req.body.get("body").is_none(), "内层化后不得再套 body");
    let upstream = req.upstream_body();
    assert!(upstream.get("messages").is_some(), "上游体含 OpenAI 字段");
    assert!(upstream.get("req_id").is_none(), "上游体无 req_id 残留");
}

/// 分发支路（dispatch 已消费协议 ID）+ 小载荷：write_chunked 产出
/// FRAME_SINGLE 首帧，必须按整条消息接受（B1/B4 借方实际线形态）。
#[tokio::test]
async fn dispatch_accepts_single_frame_request() {
    let payload = serde_json::to_vec(&sample()).expect("json");
    let expect = payload.clone();
    let (tx, rx) = tokio::io::duplex(64 * 1024);
    let mut rx: BoxedStream = Box::new(rx);
    let dial = tokio::spawn(async move {
        let mut w: BoxedStream = Box::new(tx);
        write_chunked(&mut w, &payload).await?;
        w.shutdown().await
    });
    let raw = read_request_frame(&mut rx).await.expect("request frame");
    dial.await.expect("writer task").expect("dial writes");
    assert_eq!(raw, expect, "FRAME_SINGLE 首帧须整条返回");
}

/// 分发支路 + 分片载荷：CHUNK + 带载荷 END 收束，逐段拼接。
#[tokio::test]
async fn dispatch_accepts_chunked_request() {
    let payload = serde_json::to_vec(&sample()).expect("json");
    let (head, tail) = payload.split_at(payload.len() / 2);
    let (head, tail) = (head.to_vec(), tail.to_vec());
    let (tx, rx) = tokio::io::duplex(64 * 1024);
    let mut rx: BoxedStream = Box::new(rx);
    let dial = tokio::spawn(async move {
        let mut w: BoxedStream = Box::new(tx);
        let mut chunk = vec![FRAME_CHUNK];
        chunk.extend_from_slice(&head);
        let mut end = vec![FRAME_END];
        end.extend_from_slice(&tail);
        write_frame(&mut w, &chunk).await?;
        write_frame(&mut w, &end).await?;
        w.shutdown().await
    });
    let raw = read_request_frame(&mut rx).await.expect("request frame");
    dial.await.expect("writer task").expect("dial writes");
    assert_eq!(raw, payload, "CHUNK+END 须拼接还原");
}

/// 裸流支路：首帧协议 ID 匹配后接 chunked 请求帧。
#[tokio::test]
async fn bare_stream_accepts_protocol_id_then_request() {
    let payload = serde_json::to_vec(&sample()).expect("json");
    let expect = payload.clone();
    let id = ProtocolId::new(PROTOCOL_ID).expect("protocol id");
    let (tx, rx) = tokio::io::duplex(64 * 1024);
    let mut rx: BoxedStream = Box::new(rx);
    let dial = tokio::spawn(async move {
        let mut w: BoxedStream = Box::new(tx);
        write_protocol_id(&mut w, &id).await?;
        write_chunked(&mut w, &payload).await?;
        w.shutdown().await
    });
    let raw = read_request_frame(&mut rx).await.expect("request frame");
    dial.await.expect("writer task").expect("dial writes");
    assert_eq!(raw, expect);
}

/// 类型序非法：SINGLE 出现在分片中途必须显式拒绝（对齐 read_chunked）。
#[tokio::test]
async fn dispatch_rejects_single_after_chunk() {
    let (tx, rx) = tokio::io::duplex(64 * 1024);
    let mut rx: BoxedStream = Box::new(rx);
    let dial = tokio::spawn(async move {
        let mut w: BoxedStream = Box::new(tx);
        write_frame(&mut w, &[FRAME_CHUNK, b'a']).await?;
        write_frame(&mut w, &[FRAME_SINGLE, b'b']).await?;
        w.shutdown().await
    });
    let err = read_request_frame(&mut rx)
        .await
        .expect_err("mid-chunk SINGLE must be rejected");
    dial.await.expect("writer task").expect("dial writes");
    assert!(
        err.to_string().contains("unexpected chunked frame type"),
        "err: {err}"
    );
}

/// 终结帧形状守卫：Done/Error serde 往返字段稳定（wire 契约面）。
#[test]
fn proxy_frame_roundtrip() {
    let done = ProxyFrame::Sse { d: "x".into() };
    let bytes = serde_json::to_vec(&done).expect("encode");
    let back: ProxyFrame = serde_json::from_slice(&bytes).expect("decode");
    assert!(matches!(back, ProxyFrame::Sse { .. }));
}
