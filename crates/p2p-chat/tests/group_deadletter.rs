//! goutbox 死信纪律回归（R1.1，ISSUE 33df7e4）：成员 serve 僵死（监听在但不处理，
//! 以同身份裸 Node 替身模拟——握手存活、入站群帧被静默关流，发端快速硬失败）期间
//! 两次发送硬失败，积压不得被同进程内联/后台双路径过早死信；恢复后紧邻命令内
//! 积压全量送达。红判据（R1 行为）：内联+后台对同批 failed 条目各试一次即触发
//! 「每进程二次死信」，恢复前积压被清空（backlog 断言红）、恢复后消息搁浅。

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{cleanup, peer_str, spawn, spawn_at, wait_event};
use p2p::Node;
use p2p_chat::{ChatKind, ChatStatus, GroupEvent};

/// 发端 goutbox/<peer>.jsonl 行数（0 = 该成员无积压条目）。
fn backlog(a_dir: &Path, peer: &str) -> usize {
    let path = a_dir.join("chat/goutbox").join(format!("{peer}.jsonl"));
    match std::fs::read_to_string(&path) {
        Ok(c) => c.lines().count(),
        Err(_) => 0,
    }
}

/// 僵死替身：同 data_dir（同身份）裸 Node，不挂任何聊天协议 handler。
async fn spawn_zombie(dir: &Path) -> Arc<Node> {
    Arc::new(
        Node::builder()
            .mdns(false)
            .quic_port(0)
            .tcp_port(0)
            .data_dir(dir.join("node"))
            .build()
            .await
            .expect("zombie node"),
    )
}

#[tokio::test]
async fn unreachable_window_must_not_dead_letter_backlog() {
    let a = spawn("gdl-a").await;
    let b_dir = std::env::temp_dir().join(format!("p2p-chat-gdl-b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b_dir);
    let b = spawn_at("gdl-b", &b_dir).await;
    a.chat
        .friend_add(&peer_str(&b.node), "b", b.node.listen_addrs(), None)
        .expect("a add b");
    b.chat
        .friend_add(&peer_str(&a.node), "a", a.node.listen_addrs(), None)
        .expect("b add a");
    let peer_b = peer_str(&b.node);
    let mut ev_b = b.chat.group_events();
    let g = a
        .chat
        .group
        .group_create("死信纪律群", std::slice::from_ref(&peer_b))
        .await
        .expect("create")
        .group_id;
    let joined = wait_event(
        &mut ev_b,
        |e| matches!(e, GroupEvent::State { .. }),
        "B 收 roster",
    )
    .await;
    let GroupEvent::State { group } = joined else {
        panic!("unreachable")
    };
    assert_eq!(group.group_id, g);

    // B 替换为僵死替身（同身份，只登记新地址，不触发投递）
    b.node.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let zombie = spawn_zombie(&b_dir).await;
    assert_eq!(peer_str(&zombie), peer_b, "替身同身份");
    a.chat
        .friend_add(&peer_b, "b", zombie.listen_addrs(), None)
        .expect("登记替身地址");

    // 僵死窗口：两次发送硬失败（StreamFailed 类）
    let r1 = a
        .chat
        .group
        .group_send(&g, ChatKind::Text, Some("僵死期一".into()), None, None)
        .await
        .expect("send1");
    assert_eq!(r1.acked, 0);
    let r2 = a
        .chat
        .group
        .group_send(&g, ChatKind::Text, Some("僵死期二".into()), None, None)
        .await
        .expect("send2");
    assert_eq!(r2.acked, 0);

    // —— 同步断言不给后台 flush 机会：硬失败两次后积压必须原样健在 ——
    assert_eq!(
        backlog(&a.dir, &peer_b),
        2,
        "不可达/僵死窗口禁止单进程内死信"
    );

    // 成员恢复真身 → 紧邻命令内积压全量送达
    zombie.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let b2 = spawn_at("gdl-b", &b_dir).await;
    assert_eq!(peer_str(&b2.node), peer_b, "恢复后身份不变");
    a.chat
        .friend_add(&peer_b, "b", b2.node.listen_addrs(), None)
        .expect("刷新恢复地址");
    let r3 = a
        .chat
        .group
        .group_send(&g, ChatKind::Text, Some("恢复后".into()), None, None)
        .await
        .expect("send3");
    assert!(r3.delivered, "恢复后新消息实时送达");

    // —— 同步断言：积压全量送达 + acks/状态收敛 + 出队 ——
    let hist_b = b2.chat.group.group_history(&g, None, 10).expect("b2 历史");
    let texts: Vec<_> = hist_b.iter().filter_map(|m| m.text.clone()).collect();
    for want in ["僵死期一", "僵死期二", "恢复后"] {
        assert!(
            texts.contains(&want.to_string()),
            "积压全量送达，缺 {want}：{texts:?}"
        );
    }
    let hist_a = a.chat.group.group_history(&g, None, 10).expect("a 历史");
    for want in ["僵死期一", "僵死期二", "恢复后"] {
        let m = hist_a
            .iter()
            .find(|m| m.text.as_deref() == Some(want))
            .unwrap_or_else(|| panic!("发端历史缺 {want}"));
        assert!(m.acks.contains(&peer_b), "{want} acks 未回写：{:?}", m.acks);
        assert_eq!(m.status, ChatStatus::Delivered, "{want} 状态未收敛");
    }
    assert_eq!(backlog(&a.dir, &peer_b), 0, "全量送达后出队");
    cleanup(&a);
    cleanup(&b2);
}
