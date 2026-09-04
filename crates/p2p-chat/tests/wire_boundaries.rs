//! /im/chat/1 与聊天模型异常边界（T36）：duplex 回环夹具驱动真实入站 handler，
//! 攻击端裸 Node 手写原始帧（mdns/外部端点全关）；非法输入以「断流 + 零落盘」双信号断言。

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{parse_peer, wait_until, WAIT};
use p2p::{BoxedStream, Node, ProtocolHandler, ProtocolId};
use p2p_chat::{
    sanitize_name, validate_media, validate_text, Chat, ChatKind, ChatStatus, Sender,
    CHAT_PROTOCOL, MAX_MESSAGE_SIZE as MAX,
};
use p2p_protocol::{read_frame, write_frame};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

const ENVELOPE: u8 = 0x01;
const MEDIA_BEGIN: u8 = 0x02;
const ACK: u8 = 0x04;

/// 受测端 A（装 Chat）+ 攻击端 B（裸 Node，ACK 场景换装坏 handler）。
type Fx = (Arc<Node>, Chat, Arc<Node>, PathBuf, String, String);

async fn spawn_node(dir: &Path) -> Arc<Node> {
    let built = Node::builder()
        .mdns(false)
        .data_dir(dir.to_path_buf())
        .build()
        .await;
    Arc::new(built.expect("构建回环节点"))
}

async fn fx(tag: &str) -> Fx {
    let dir = std::env::temp_dir().join(format!("p2p-chat-bound-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = spawn_node(&dir.join("a")).await;
    let chat = Chat::new(a.clone(), dir.join("a")).expect("装配 Chat");
    let b = spawn_node(&dir.join("b")).await;
    let peer_a = a.local_peer_id().to_string();
    let peer_b = b.local_peer_id().to_string();
    for addr in a.listen_addrs() {
        b.add_peer_address(a.local_peer_id(), &addr)
            .expect("登记 A 地址");
    }
    (a, chat, b, dir, peer_a, peer_b)
}

fn done(a: &Node, b: &Node, dir: &Path) {
    a.shutdown();
    b.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

fn chat_proto() -> ProtocolId {
    ProtocolId::new(CHAT_PROTOCOL).expect("协议 id")
}

/// B → A 开 /im/chat/1 流（先幂等 connect，再开业务流）。
async fn open_stream(b: &Node, peer_a: &str) -> BoxedStream {
    let peer = parse_peer(peer_a);
    b.connect(peer).await.expect("B 连接 A");
    b.new_stream(peer, chat_proto())
        .await
        .expect("开 /im/chat/1 流")
}

async fn write_typed(stream: &mut BoxedStream, kind: u8, payload: &[u8]) {
    let mut frame = Vec::with_capacity(payload.len() + 1);
    frame.push(kind);
    frame.extend_from_slice(payload);
    write_frame(stream, &frame).await.expect("写帧");
}

/// 线上信封基线（camelCase + tsMs，对齐 wire-protocol.md §8.1）。
fn envelope(peer: &str) -> Value {
    json!({"id": "m-1", "peer": peer, "sender": "me", "kind": "text", "tsMs": 7, "text": "你好", "media": null})
}

/// 信封字段补丁（返回线上 JSON 载荷）。
fn env_with(peer: &str, patch: Value) -> String {
    let mut env = envelope(peer);
    if let Value::Object(fields) = patch {
        for (key, value) in fields {
            env[key] = value;
        }
    }
    env.to_string()
}

/// 非法输入的可观测失败信号：对端断流 + 受测端零落盘。
async fn expect_rejected(chat: &Chat, peer_b: &str, stream: &mut BoxedStream, what: &str) {
    let read = tokio::time::timeout(WAIT, read_frame(stream)).await;
    let got = read.unwrap_or_else(|_| panic!("{what}: 受测端未在期限内断流"));
    assert!(got.is_err(), "{what}: 非法帧必须断流，却收到响应 {got:?}");
    let msgs = chat.history(peer_b, None, 10).expect("读历史");
    assert!(msgs.is_empty(), "{what}: 非法输入不得落盘 {msgs:?}");
}

#[test]
fn pure_validation_and_sanitize_boundaries() {
    assert!(
        validate_text("").is_err() && validate_text("   \n\t").is_err(),
        "空文本必须拒绝"
    );
    assert_eq!(validate_text("  hi  ").expect("trim 后合法"), "hi");
    assert!(
        validate_text(&"汉".repeat(2000)).is_ok(),
        "2000 字符边界必须通过"
    );
    assert!(
        validate_text(&"汉".repeat(2001)).is_err(),
        "2001 字符必须拒绝"
    );
    assert!(
        validate_media(&ChatKind::Image, "image/png", 0).is_err(),
        "零字节附件必须拒绝"
    );
    assert!(
        validate_media(&ChatKind::Image, "image/png", MAX).is_ok(),
        "恰好 64MiB 必须通过"
    );
    assert!(
        validate_media(&ChatKind::File, "application/octet-stream", MAX + 1).is_err(),
        "超 64MiB 必须拒绝"
    );
    assert!(validate_media(&ChatKind::Image, "image/webp", 1).is_ok());
    assert!(validate_media(&ChatKind::Audio, "audio/m4a", 1).is_ok());
    assert!(validate_media(&ChatKind::Video, "video/quicktime", 1).is_ok());
    assert!(
        validate_media(&ChatKind::Image, "IMAGE/PNG", 1).is_ok(),
        "mime 大小写不敏感"
    );
    assert!(
        validate_media(&ChatKind::Image, "image/svg+xml", 1).is_err(),
        "白名单外必须拒绝"
    );
    assert!(
        validate_media(&ChatKind::Image, "text/plain", 1).is_err(),
        "跨类 MIME 必须拒绝"
    );
    assert_eq!(sanitize_name("a/b\\c:d"), "abcd", "路径分隔符必须剥离");
    assert_eq!(sanitize_name("my file.png"), "myfile.png", "空格必须剥离");
    assert_eq!(sanitize_name("中文名.png"), ".png", "非 ASCII 必须剥离");
    assert_eq!(
        sanitize_name("../../etc/passwd"),
        "....etcpasswd",
        "相对路径必须失效"
    );
}

#[tokio::test]
async fn inbound_bad_frames_close_stream_without_persist() {
    let (a, chat, b, dir, peer_a, peer_b) = fx("reject").await;
    let huge = json!({"kind":"image","media":{"name":"a","mime":"image/png","size":MAX + 1}});
    let media4 = json!({"kind":"image","media":{"name":"a","mime":"image/png","size":4}});
    let head = br#"{"len":1,"name":"a.png","mime":"image/png","kind":"image"}"#;
    let cases: Vec<(&str, u8, String)> = vec![
        ("未知帧类型 0x7f", 0x7f, "x".into()),
        ("首帧 MEDIA_BEGIN 乱序", MEDIA_BEGIN, "x".into()),
        ("空信封", ENVELOPE, String::new()),
        ("非法 JSON 信封", ENVELOPE, "not-json".into()),
        ("缺字段信封", ENVELOPE, "{}".into()),
        (
            "sender 非 me 伪装",
            ENVELOPE,
            env_with(&peer_b, json!({"sender": "them"})),
        ),
        ("peer 指向本机伪装", ENVELOPE, envelope(&peer_a).to_string()),
        ("附件声明超 64MiB", ENVELOPE, env_with(&peer_b, huge)),
    ];
    for (what, kind, payload) in cases {
        let mut stream = open_stream(&b, &peer_a).await;
        write_typed(&mut stream, kind, payload.as_bytes()).await;
        expect_rejected(&chat, &peer_b, &mut stream, what).await;
    }
    // 媒体头 len 与信封 size 不一致 → 断流
    let mut stream = open_stream(&b, &peer_a).await;
    write_typed(&mut stream, ENVELOPE, env_with(&peer_b, media4).as_bytes()).await;
    write_typed(&mut stream, MEDIA_BEGIN, head).await;
    expect_rejected(&chat, &peer_b, &mut stream, "媒体长度不一致").await;
    // 帧超限：手写 varint 长度前缀（1MiB+1 = [0x81, 0x80, 0xC0, 0x00]），读端长度阶段即拒
    let mut stream = open_stream(&b, &peer_a).await;
    stream
        .write_all(&[0x81, 0x80, 0xC0, 0x00])
        .await
        .expect("写超限长度前缀");
    stream.write_all(&[0xab; 16]).await.expect("写帧体占位");
    expect_rejected(&chat, &peer_b, &mut stream, "帧超限").await;
    done(&a, &b, &dir);
}

/// 写合法信封并读取对端 ACK（duplex 正向通道）。
async fn deliver_raw(b: &Node, peer_a: &str, body: &[u8]) -> Value {
    let mut stream = open_stream(b, peer_a).await;
    write_typed(&mut stream, ENVELOPE, body).await;
    let frame = tokio::time::timeout(WAIT, read_frame(&mut stream)).await;
    let frame = frame.expect("ACK 超时").expect("ACK 读取");
    assert_eq!(frame[0], ACK, "首响应必须是 ACK 帧");
    serde_json::from_slice(&frame[1..]).expect("ACK JSON")
}

#[tokio::test]
async fn valid_delivery_ack_then_duplicate_id_is_idempotent() {
    let (a, chat, b, dir, peer_a, peer_b) = fx("dup").await;
    let body = envelope(&peer_b).to_string();
    let ack = deliver_raw(&b, &peer_a, body.as_bytes()).await;
    assert_eq!(ack["id"], json!("m-1"), "ACK 应回显信封 id");
    assert_eq!(ack["ok"], json!(true), "合法信封必须 ACK");
    wait_until("落盘", || {
        chat.history(&peer_b, None, 10).is_ok_and(|h| h.len() == 1)
    })
    .await;
    let stored = chat.history(&peer_b, None, 10).expect("读历史");
    assert_eq!(stored[0].sender, Sender::Them, "入站 sender 应为 them");
    assert_eq!(stored[0].status, ChatStatus::Delivered, "入站应 delivered");
    // 同 id 重发：仍回 ACK，但只落盘一次（wire-protocol §8.1 幂等）
    let ack2 = deliver_raw(&b, &peer_a, body.as_bytes()).await;
    assert_eq!(ack2["ok"], json!(true), "重复投递仍应 ACK");
    assert_eq!(
        chat.history(&peer_b, None, 10).expect("读历史").len(),
        1,
        "重复消息不得重复落盘"
    );
    done(&a, &b, &dir);
}

/// 坏对端 handler：读信封后回 ok=false；true 时回非 ACK 帧（经 handle_protocol 换装）。
struct BadPeer(bool);

#[async_trait::async_trait]
impl ProtocolHandler for BadPeer {
    fn protocol(&self) -> ProtocolId {
        chat_proto()
    }

    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        read_frame(&mut stream).await?;
        let mut payload = vec![if self.0 { 0x7f } else { ACK }];
        if !self.0 {
            let ack = json!({"id": "not-the-sent-id", "ok": false, "reason": "对端拒绝"});
            payload.extend_from_slice(ack.to_string().as_bytes());
        }
        write_frame(&mut stream, &payload).await
    }
}

#[tokio::test]
async fn bad_ack_variants_fail_delivery_and_mark_failed() {
    for (i, not_ack_frame) in [false, true].into_iter().enumerate() {
        let what = if not_ack_frame {
            "非 ACK 帧"
        } else {
            "ACK ok=false"
        };
        let (a, chat, b, dir, _peer_a, peer_b) = fx(&format!("ack{i}")).await;
        b.handle_protocol(Arc::new(BadPeer(not_ack_frame)));
        chat.friend_add(&peer_b, "b", b.listen_addrs(), None)
            .expect("登记 B 地址");
        let sent = chat
            .send(&peer_b, ChatKind::Text, Some("hi".into()), None)
            .await;
        let report = sent.expect("send 应返回报告而非 Err");
        assert!(!report.delivered, "{what}: 坏 ACK 不得计送达");
        assert_eq!(
            report.message.status,
            ChatStatus::Failed,
            "{what}: 应 failed"
        );
        let stored = chat.history(&peer_b, None, 10).expect("读历史");
        assert_eq!(stored[0].status, ChatStatus::Failed, "{what}: 落盘可观测");
        done(&a, &b, &dir);
    }
}
