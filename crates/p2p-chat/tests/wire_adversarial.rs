//! /im/chat/1 对抗性边界（IM-T49 采纳自 feat/t36-chat-boundary-tests 11fe0a6，适配
//! T46A replyTo 线上契约）：duplex 回环夹具驱动真实入站 handler，攻击端裸 Node
//! 手写原始帧；非法输入以「断流 + 零落盘」双信号断言；另证 wire 级重复 id 幂等
//! 与坏 ACK（ok=false / 非 ACK 帧）的 failed 处置。

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{parse_peer, wait_until, WAIT};
use p2p::{BoxedStream, Node, ProtocolHandler, ProtocolId};
use p2p_chat::{Chat, ChatKind, ChatStatus, Sender, CHAT_PROTOCOL, MAX_MESSAGE_SIZE as MAX};
use p2p_protocol::{read_frame, write_frame};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

const ENVELOPE: u8 = 0x01;
const MEDIA_BEGIN: u8 = 0x02;
const ACK: u8 = 0x04;

/// 受测端 A（装 Chat）+ 攻击端 B（裸 Node；坏 ACK 场景换装 handler）。
struct Fx {
    a: Arc<Node>,
    chat: Chat,
    b: Arc<Node>,
    dir: PathBuf,
    peer_a: String,
    peer_b: String,
}

async fn spawn_node(dir: &Path) -> Arc<Node> {
    let built = Node::builder()
        .mdns(false)
        .quic_port(0)
        .tcp_port(0)
        .data_dir(dir.to_path_buf())
        .build()
        .await;
    Arc::new(built.expect("构建回环节点"))
}

async fn fx(tag: &str) -> Fx {
    let dir = std::env::temp_dir().join(format!("p2p-chat-adv-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = spawn_node(&dir.join("a")).await;
    let chat = Chat::new(a.clone(), dir.join("a")).expect("装配 Chat");
    let b = spawn_node(&dir.join("b")).await;
    for addr in a.listen_addrs() {
        b.add_peer_address(a.local_peer_id(), &addr)
            .expect("登记 A 地址");
    }
    Fx {
        peer_a: a.local_peer_id().to_string(),
        peer_b: b.local_peer_id().to_string(),
        a,
        chat,
        b,
        dir,
    }
}

fn done(fx: Fx) {
    fx.a.shutdown();
    fx.b.shutdown();
    let _ = std::fs::remove_dir_all(fx.dir);
}

fn chat_proto() -> ProtocolId {
    ProtocolId::new(CHAT_PROTOCOL).expect("协议 id")
}

/// B → A 开 /im/chat/1 流（先幂等 connect，再开业务流）。
async fn open_stream(fx: &Fx) -> BoxedStream {
    let peer = parse_peer(&fx.peer_a);
    fx.b.connect(peer).await.expect("B 连接 A");
    fx.b.new_stream(peer, chat_proto())
        .await
        .expect("开 /im/chat/1 流")
}

async fn write_typed(stream: &mut BoxedStream, kind: u8, payload: &[u8]) {
    let mut frame = Vec::with_capacity(payload.len() + 1);
    frame.push(kind);
    frame.extend_from_slice(payload);
    write_frame(stream, &frame).await.expect("写帧");
}

/// 线上信封基线（camelCase + tsMs + replyTo，对齐 wire-protocol.md §8.1）。
fn envelope(peer: &str) -> Value {
    json!({"id": "m-1", "peer": peer, "sender": "me", "kind": "text",
        "tsMs": 7, "text": "你好", "media": null, "replyTo": null})
}

/// 信封字段补丁（返回线上 JSON 载荷字符串）。
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
async fn expect_rejected(fx: &Fx, stream: &mut BoxedStream, what: &str) {
    let read = tokio::time::timeout(WAIT, read_frame(stream)).await;
    let got = read.unwrap_or_else(|_| panic!("{what}: 受测端未在期限内断流"));
    assert!(got.is_err(), "{what}: 非法帧必须断流，却收到响应 {got:?}");
    let msgs = fx.chat.history(&fx.peer_b, None, 10).expect("读历史");
    assert!(msgs.is_empty(), "{what}: 非法输入不得落盘 {msgs:?}");
}

/// 对抗用例表：各例均带 replyTo:null（缺字段例单独覆盖 T46A 新契约）。
fn reject_cases(peer_a: &str, peer_b: &str) -> Vec<(&'static str, u8, String)> {
    let no_reply_to = r#"{"id":"m-1","peer":PEER,"sender":"me","kind":"text","tsMs":7,"text":"你好","media":null}"#
        .replace("PEER", peer_b);
    vec![
        ("未知帧类型 0x7f", 0x7f, "x".into()),
        ("首帧 MEDIA_BEGIN 乱序", MEDIA_BEGIN, "x".into()),
        ("空信封", ENVELOPE, String::new()),
        ("非法 JSON 信封", ENVELOPE, "not-json".into()),
        ("缺字段信封", ENVELOPE, "{}".into()),
        ("缺 replyTo 字段", ENVELOPE, no_reply_to),
        (
            "sender 非 me 伪装",
            ENVELOPE,
            env_with(peer_b, json!({"sender": "them"})),
        ),
        ("peer 指向本机伪装", ENVELOPE, envelope(peer_a).to_string()),
        (
            "附件声明超 64MiB",
            ENVELOPE,
            env_with(
                peer_b,
                json!({"kind":"image","media":{"name":"a","mime":"image/png","size":MAX + 1}}),
            ),
        ),
    ]
}

#[tokio::test]
async fn inbound_bad_frames_close_stream_without_persist() {
    let fx = fx("reject").await;
    for (what, kind, payload) in reject_cases(&fx.peer_a, &fx.peer_b) {
        let mut stream = open_stream(&fx).await;
        write_typed(&mut stream, kind, payload.as_bytes()).await;
        expect_rejected(&fx, &mut stream, what).await;
    }
    // 媒体头 len 与信封 size 不一致 → 断流
    let media4 = json!({"kind":"image","media":{"name":"a","mime":"image/png","size":4}});
    let mut stream = open_stream(&fx).await;
    write_typed(
        &mut stream,
        ENVELOPE,
        env_with(&fx.peer_b, media4).as_bytes(),
    )
    .await;
    write_typed(
        &mut stream,
        MEDIA_BEGIN,
        br#"{"len":1,"name":"a.png","mime":"image/png","kind":"image"}"#,
    )
    .await;
    expect_rejected(&fx, &mut stream, "媒体长度不一致").await;
    // 帧超限：手写 varint 长度前缀（1MiB+1 = [0x81, 0x80, 0xC0, 0x00]），读端长度阶段即拒
    let mut stream = open_stream(&fx).await;
    stream
        .write_all(&[0x81, 0x80, 0xC0, 0x00])
        .await
        .expect("写超限长度前缀");
    stream.write_all(&[0xab; 16]).await.expect("写帧体占位");
    expect_rejected(&fx, &mut stream, "帧超限").await;
    done(fx);
}

/// 写合法信封并读取对端 ACK（duplex 正向通道）。
async fn deliver_raw(fx: &Fx, body: &[u8]) -> Value {
    let mut stream = open_stream(fx).await;
    write_typed(&mut stream, ENVELOPE, body).await;
    let frame = tokio::time::timeout(WAIT, read_frame(&mut stream)).await;
    let frame = frame.expect("ACK 超时").expect("ACK 读取");
    assert_eq!(frame[0], ACK, "首响应必须是 ACK 帧");
    serde_json::from_slice(&frame[1..]).expect("ACK JSON")
}

#[tokio::test]
async fn valid_delivery_ack_then_duplicate_id_is_idempotent() {
    let fx = fx("dup").await;
    let body = envelope(&fx.peer_b).to_string();
    let ack = deliver_raw(&fx, body.as_bytes()).await;
    assert_eq!(ack["id"], json!("m-1"), "ACK 应回显信封 id");
    assert_eq!(ack["ok"], json!(true), "合法信封必须 ACK");
    wait_until("落盘", || {
        fx.chat
            .history(&fx.peer_b, None, 10)
            .is_ok_and(|h| h.len() == 1)
    })
    .await;
    let stored = fx.chat.history(&fx.peer_b, None, 10).expect("读历史");
    assert_eq!(stored[0].sender, Sender::Them, "入站 sender 应为 them");
    assert_eq!(stored[0].status, ChatStatus::Delivered, "入站应 delivered");
    // 同 id 重发：仍回 ACK，但只落盘一次（wire-protocol §8.1 幂等）
    let ack2 = deliver_raw(&fx, body.as_bytes()).await;
    assert_eq!(ack2["ok"], json!(true), "重复投递仍应 ACK");
    assert_eq!(
        fx.chat.history(&fx.peer_b, None, 10).expect("读历史").len(),
        1,
        "重复消息不得重复落盘"
    );
    done(fx);
}

/// 坏对端 handler：读信封后回非 ACK 帧（true）或 ok=false ACK（false）。
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
        let fx = fx(&format!("ack{i}")).await;
        fx.b.handle_protocol(Arc::new(BadPeer(not_ack_frame)));
        fx.chat
            .friend_add_direct(&fx.peer_b, "b", fx.b.listen_addrs(), None)
            .expect("登记 B 地址");
        let report = fx
            .chat
            .send(&fx.peer_b, ChatKind::Text, Some("hi".into()), None, None)
            .await;
        let report = report.expect("send 应返回报告而非 Err");
        assert!(!report.delivered, "{what}: 坏 ACK 不得计送达");
        assert_eq!(
            report.message.status,
            ChatStatus::Failed,
            "{what}: 应 failed"
        );
        let stored = fx.chat.history(&fx.peer_b, None, 10).expect("读历史");
        assert_eq!(stored[0].status, ChatStatus::Failed, "{what}: 落盘可观测");
        done(fx);
    }
}
