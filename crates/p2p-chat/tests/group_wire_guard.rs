//! /im/group/1 对抗性边界（design §3.2/§3.3）：攻击端裸 Node 手写原始帧。
//! 覆盖：roster rev 收敛与幂等丢弃、owner 绑定拒收、members 不含本机拒收、
//! unknown_group ACK 拒绝路径、(groupId,id) 去重、sender ∉ 成员断流。

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{parse_peer, WAIT};
use p2p::{BoxedStream, Node, ProtocolId};
use p2p_chat::GROUP_PROTOCOL;
use p2p_protocol::{read_frame, write_frame};
use serde_json::{json, Value};

/// 受测端 A（装 Chat）+ 攻击端 B（装 Chat 以收 roster，攻击帧为手写原始帧）。
struct Fx {
    chat: p2p_chat::Chat,
    a: Arc<Node>,
    b: Arc<Node>,
    _chat_b: p2p_chat::Chat,
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
    Arc::new(built.expect("构建节点"))
}

async fn fx(tag: &str) -> Fx {
    let dir = std::env::temp_dir().join(format!("p2p-chat-gw-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = spawn_node(&dir.join("a")).await;
    let chat = p2p_chat::Chat::new(a.clone(), dir.join("a")).expect("装配 Chat");
    let b = spawn_node(&dir.join("b")).await;
    let chat_b = p2p_chat::Chat::new(b.clone(), dir.join("b")).expect("装配 B Chat");
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
        _chat_b: chat_b,
        dir,
    }
}

fn done(fx: Fx) {
    fx.a.shutdown();
    fx.b.shutdown();
    let _ = std::fs::remove_dir_all(fx.dir);
}

fn group_proto() -> ProtocolId {
    ProtocolId::new(GROUP_PROTOCOL).expect("协议 id")
}

async fn open_stream(fx: &Fx) -> BoxedStream {
    let peer = parse_peer(&fx.peer_a);
    fx.b.connect(peer).await.expect("B 连接 A");
    fx.b.new_stream(peer, group_proto())
        .await
        .expect("开 /im/group/1 流")
}

async fn write_typed(stream: &mut BoxedStream, kind: u8, payload: &[u8]) {
    let mut frame = Vec::with_capacity(payload.len() + 1);
    frame.push(kind);
    frame.extend_from_slice(payload);
    write_frame(stream, &frame).await.expect("写帧");
}

const G_ENVELOPE: u8 = 0x01;
const G_STATE: u8 = 0x11;
const G_STATE_ACK: u8 = 0x12;
const G_KICK: u8 = 0x13;

fn roster(group_id: &str, owner: &str, members: Value, rev: u64) -> String {
    json!({"groupId": group_id, "name": "攻防群", "owner": owner, "members": members,
        "rev": rev, "tsMs": 1})
    .to_string()
}

/// 推 roster 并读回 G_STATE_ACK。
async fn push_roster(fx: &Fx, body: String) -> Value {
    let mut stream = open_stream(fx).await;
    write_typed(&mut stream, G_STATE, body.as_bytes()).await;
    let frame = tokio::time::timeout(WAIT, read_frame(&mut stream))
        .await
        .expect("ACK 超时")
        .expect("读 G_STATE_ACK");
    assert_eq!(frame[0], G_STATE_ACK, "首响应必须是 G_STATE_ACK");
    serde_json::from_slice(&frame[1..]).expect("ACK JSON")
}

fn group_of(fx: &Fx, group_id: &str) -> Option<p2p_chat::GroupInfo> {
    fx.chat
        .group
        .group_list()
        .into_iter()
        .find(|g| g.group_id == group_id)
}

/// roster rev 收敛：高 rev 胜；≤ 本地幂等丢弃（ok=true 不变更）；owner 绑定不符拒收；
/// members 不含本机/重复/超员拒收（ok=false，群状态不变）。
#[tokio::test]
async fn roster_rev_convergence_and_owner_binding() {
    let fx = fx("roster").await;
    let gid = uuid::Uuid::new_v4().to_string();
    let stranger = p2p_identity::Keypair::generate().peer_id().to_string();

    let ack = push_roster(&fx, roster(&gid, &fx.peer_b, json!([fx.peer_a]), 5)).await;
    assert_eq!(ack["ok"], json!(true), "首见 roster 接受");
    assert_eq!(ack["rev"], json!(5));
    let g = group_of(&fx, &gid).expect("群已落盘");
    assert_eq!(g.owner, fx.peer_b, "首见落定 owner");
    assert_eq!(g.rev, 5);

    // 等 rev 幂等丢弃：ok=true 但内容不变
    let ack = push_roster(&fx, roster(&gid, &fx.peer_b, json!([fx.peer_a]), 5)).await;
    assert_eq!(ack["ok"], json!(true));
    assert_eq!(group_of(&fx, &gid).expect("g").name, "攻防群");

    // 低 rev 幂等丢弃
    let ack = push_roster(&fx, roster(&gid, &fx.peer_b, json!([fx.peer_a]), 3)).await;
    assert_eq!(ack["ok"], json!(true), "低 rev 幂等 ok");
    assert_eq!(group_of(&fx, &gid).expect("g").rev, 5, "rev 不回退");

    // owner 绑定不符拒收：owner 换人
    let ack = push_roster(&fx, roster(&gid, &stranger, json!([fx.peer_a]), 9)).await;
    assert_eq!(ack["ok"], json!(false), "owner 绑定拒收");
    assert_eq!(group_of(&fx, &gid).expect("g").rev, 5);

    // owner 指向本机拒收
    let ack = push_roster(&fx, roster(&gid, &fx.peer_a, json!([fx.peer_a]), 9)).await;
    assert_eq!(ack["ok"], json!(false), "owner = 本机拒收");

    // members 不含本机拒收；重复项拒收
    let ack = push_roster(&fx, roster(&gid, &fx.peer_b, json!([stranger]), 9)).await;
    assert_eq!(ack["ok"], json!(false), "members 不含本机拒收");
    let ack = push_roster(
        &fx,
        roster(&gid, &fx.peer_b, json!([fx.peer_a, fx.peer_a]), 9),
    )
    .await;
    assert_eq!(ack["ok"], json!(false), "members 重复拒收");

    // 合法高 rev 收敛
    let ack = push_roster(
        &fx,
        json!({"groupId": gid, "name": "攻防群2", "owner": fx.peer_b,
            "members": [fx.peer_a, fx.peer_b], "rev": 9, "tsMs": 2})
        .to_string(),
    )
    .await;
    assert_eq!(ack["ok"], json!(true));
    let g = group_of(&fx, &gid).expect("g");
    assert_eq!(g.rev, 9);
    assert_eq!(g.name, "攻防群2");
    done(fx);
}

/// 消息事务：未知群回 unknown_group（发端保持 pending）；合法消息去重；
/// sender ∉ 成员断流；G_KICK 置位被踢者状态。
#[tokio::test]
async fn unknown_group_reject_and_message_dedup() {
    let fx = fx("msg").await;
    let gid = uuid::Uuid::new_v4().to_string();

    // unknown_group：回 ACK ok=false reason=unknown_group（非断流），零落盘
    let env = json!({"id": "u-1", "groupId": gid, "sender": fx.peer_b, "kind": "text",
        "tsMs": 7, "text": "在吗", "media": null, "replyTo": null});
    let mut stream = open_stream(&fx).await;
    write_typed(&mut stream, G_ENVELOPE, env.to_string().as_bytes()).await;
    let frame = tokio::time::timeout(WAIT, read_frame(&mut stream))
        .await
        .expect("ACK 超时")
        .expect("读 ACK");
    assert_eq!(frame[0], 0x04, "未知群仍回 ACK 帧");
    let ack: Value = serde_json::from_slice(&frame[1..]).expect("ACK JSON");
    assert_eq!(ack["ok"], json!(false));
    assert_eq!(ack["reason"], json!("unknown_group"));
    assert!(
        fx.chat
            .group
            .group_history(&gid, None, 10)
            .is_ok_and(|h| h.is_empty()),
        "未知群零落盘"
    );

    // A 建 [B]：B 入好友簿与群（roster 推 B 失败可忽略——B 无 handler）
    fx.chat
        .friend_add(&fx.peer_b, "b", fx.b.listen_addrs(), None)
        .expect("a add b");
    let g = fx
        .chat
        .group
        .group_create("去重群", std::slice::from_ref(&fx.peer_b))
        .await
        .expect("create");

    // 合法消息落盘；同 id 重发仅回 ACK 不重复落盘
    let env = json!({"id": "m-1", "groupId": g.group_id, "sender": fx.peer_b, "kind": "text",
        "tsMs": 7, "text": "hello", "media": null, "replyTo": null});
    for round in 0..2 {
        let mut stream = open_stream(&fx).await;
        write_typed(&mut stream, G_ENVELOPE, env.to_string().as_bytes()).await;
        let frame = tokio::time::timeout(WAIT, read_frame(&mut stream))
            .await
            .expect("ACK 超时")
            .expect("读 ACK");
        let ack: Value = serde_json::from_slice(&frame[1..]).expect("ACK JSON");
        assert_eq!(ack["ok"], json!(true), "第 {round} 轮投递必须 ACK");
        assert_eq!(ack["id"], json!("m-1"));
    }
    assert_eq!(
        fx.chat
            .group
            .group_history(&g.group_id, None, 10)
            .expect("history")
            .len(),
        1,
        "(groupId,id) 去重只落一次"
    );

    // sender ∉ 成员：断流（读端直接失败），零新增
    let env = json!({"id": "m-2", "groupId": g.group_id, "sender": stranger_of(&fx), "kind": "text",
        "tsMs": 7, "text": "伪装", "media": null, "replyTo": null});
    let mut stream = open_stream(&fx).await;
    write_typed(&mut stream, G_ENVELOPE, env.to_string().as_bytes()).await;
    let read = tokio::time::timeout(WAIT, read_frame(&mut stream)).await;
    assert!(
        read.is_err() || read.expect("无超时").is_err(),
        "非成员必须断流"
    );
    assert_eq!(
        fx.chat
            .group
            .group_history(&g.group_id, None, 10)
            .expect("history")
            .len(),
        1,
        "伪装消息不落盘"
    );

    // G_KICK：A 是 owner → 外来 kick 告警忽略，状态不变（roster 才是权威）
    let kick = json!({"groupId": g.group_id, "rev": 99, "reason": "kicked"}).to_string();
    let mut stream = open_stream(&fx).await;
    write_typed(&mut stream, G_KICK, kick.as_bytes()).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(
        group_of(&fx, &g.group_id).expect("g").state,
        p2p_chat::GroupState::Active
    );
    assert_eq!(
        fx.chat
            .group
            .group_history(&g.group_id, None, 10)
            .expect("history")
            .len(),
        1,
        "历史不受影响"
    );
    done(fx);
}

fn stranger_of(fx: &Fx) -> String {
    let _ = fx;
    p2p_identity::Keypair::generate().peer_id().to_string()
}
