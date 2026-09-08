//! A2A2a card 链 E2E（a2a-over-p2p-design §11 验收）：真 Node QUIC loopback
//! 双节点——宿主经 admin HTTP 管理 agent，访客 list 仅见 public、验签入簿、
//! subscribe 应答含快照、删除后收 removed 推送。卡片链不涉子进程。

mod a2a_card_common;

use a2a::{AgentBook, CardFrame, Visibility};
use p2p::Node;

use a2a_card_common::{admin_delete, guest_stream, host_rig, read_frame_json, send_frame};

#[tokio::test]
async fn t1_list_visibility_verify_and_book() {
    let host = host_rig("t1").await;
    host.agents
        .create(
            Some("pub-agent".into()),
            "公开评审".into(),
            "desc".into(),
            vec![],
            Visibility::Public,
            1,
        )
        .expect("create public");
    host.agents
        .create(
            Some("priv-agent".into()),
            "私有助理".into(),
            "desc".into(),
            vec![],
            Visibility::Private,
            1,
        )
        .expect("create private");
    let guest = Node::builder().mdns(false).build().await.expect("guest");
    let mut stream = guest_stream(&guest, &host.peer, &host.node.listen_addrs()).await;
    send_frame(&mut stream, &CardFrame::List { v: 1, id: 1 }).await;
    let reply = read_frame_json(&mut stream).await;
    let CardFrame::Cards { id, cards, .. } = reply else {
        panic!("期望 Cards 应答: {reply:?}");
    };
    assert_eq!(id, 1);
    assert_eq!(cards.len(), 1, "远程仅见 public（fail-closed）");
    assert_eq!(cards[0].0.payload.agent_id, "pub-agent");
    // 验签 + 入簿（订阅侧 AgentBook 纪律）
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut book = AgentBook::new();
    for card in &cards {
        card.verify(now).expect("card verify");
        book.insert(card.clone(), now).expect("book insert");
    }
    assert_eq!(book.len(), 1);
    guest.shutdown();
}

#[tokio::test]
async fn t2_get_private_not_found() {
    let host = host_rig("t2").await;
    host.agents
        .create(
            Some("priv-x".into()),
            "私有".into(),
            "d".into(),
            vec![],
            Visibility::Private,
            1,
        )
        .expect("create private");
    let guest = Node::builder().mdns(false).build().await.expect("guest");
    let mut stream = guest_stream(&guest, &host.peer, &host.node.listen_addrs()).await;
    send_frame(
        &mut stream,
        &CardFrame::Get {
            v: 1,
            id: 7,
            agent_id: "priv-x".into(),
        },
    )
    .await;
    let reply = read_frame_json(&mut stream).await;
    let CardFrame::Error { id, code, .. } = reply else {
        panic!("期望 Error 应答: {reply:?}");
    };
    assert_eq!(id, 7);
    assert_eq!(code, "not-found");
    guest.shutdown();
}

#[tokio::test]
async fn t3_subscribe_snapshot_then_remove_push() {
    let host = host_rig("t3").await;
    let def = host
        .agents
        .create(
            Some("pub-s".into()),
            "订阅测".into(),
            "d".into(),
            vec![],
            Visibility::Public,
            1,
        )
        .expect("create");
    let guest = Node::builder().mdns(false).build().await.expect("guest");
    let mut stream = guest_stream(&guest, &host.peer, &host.node.listen_addrs()).await;
    send_frame(&mut stream, &CardFrame::Subscribe { v: 1, id: 3 }).await;
    // 应答 1: Ok{subscribed:true}
    let ok = read_frame_json(&mut stream).await;
    assert!(
        matches!(
            ok,
            CardFrame::Ok {
                id: 3,
                subscribed: true,
                ..
            }
        ),
        "{ok:?}"
    );
    // 应答 2: 快照 Push（含当前卡）
    let snapshot = read_frame_json(&mut stream).await;
    let CardFrame::Push { cards, removed, .. } = snapshot else {
        panic!("期望快照 Push: {snapshot:?}");
    };
    assert_eq!(cards.len(), 1);
    assert!(removed.is_empty());
    // admin HTTP 删除 → 订阅侧收到 removed 推送
    let (status, body) = admin_delete(host.admin_addr, &host.admin_token, &def.agent_id).await;
    assert_eq!(status, 200, "admin delete: {body}");
    let push = read_frame_json(&mut stream).await;
    let CardFrame::Push { cards, removed, .. } = push else {
        panic!("期望 removed Push: {push:?}");
    };
    assert!(cards.is_empty());
    assert_eq!(removed, vec![format!("{}/pub-s", host.peer)]);
    guest.shutdown();
}
