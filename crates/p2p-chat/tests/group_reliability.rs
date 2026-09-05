//! 群聊可靠性回归（ISSUE 2026-09-05 演练前三缺陷，R1 任务书）：
//! a) 离线成员补投在「成员重新在线后的下一次发送命令」内送达——紧邻一步，非数步后；
//! b) 补投送达后发送端历史 acks 含全员、状态收敛 delivered、goutbox 出队无双账本；
//! c) roster 推送 failed 条目在成员重新可达后的下一次命令内重投成功、成员 rev 收敛。
//! 红绿判据确定性：send 返回后到断言之间不落任何 await 点，当前线程调度下
//! 后台 PeerConnected flush 物理上不可能插队运行，补投只能来自命令内联路径。

mod common;

use std::path::{Path, PathBuf};

use common::{add_each_other, cleanup, peer_str, spawn, spawn_at, wait_event, TestNode};
use p2p_chat::{ChatKind, GroupEvent};

/// 发端 goutbox/<peer>.jsonl 行数（0 = 该成员无积压条目）。
fn backlog(a_dir: &Path, peer: &str) -> usize {
    let path = a_dir.join("chat/goutbox").join(format!("{peer}.jsonl"));
    match std::fs::read_to_string(&path) {
        Ok(c) => c.lines().count(),
        Err(_) => 0,
    }
}

/// 三方装配：A owner + B/C 成员全员在线建群 → 确认 B/C 在册 → 关停 C（离线）。
/// 返回 (a, b, c 的数据目录, peer_c)；c 重启由用例在恰当时机执行。
async fn group_with_c_offline(tag: &str) -> (TestNode, TestNode, PathBuf, String) {
    let a = spawn(&format!("{tag}-a")).await;
    let b = spawn(&format!("{tag}-b")).await;
    let c_dir = std::env::temp_dir().join(format!("p2p-chat-{tag}-c-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c_dir);
    let c = spawn_at(&format!("{tag}-c"), &c_dir).await;
    add_each_other(&a, &b).await;
    a.chat
        .friend_add(&peer_str(&c.node), "c", c.node.listen_addrs(), None)
        .expect("a add c");
    c.chat
        .friend_add(&peer_str(&a.node), "a", a.node.listen_addrs(), None)
        .expect("c add a");
    let peer_c = peer_str(&c.node);
    let mut ev_b = b.chat.group_events();
    let g = a
        .chat
        .group
        .group_create("可靠性群", &[peer_str(&b.node), peer_c.clone()])
        .await
        .expect("create");
    let joined = wait_event(
        &mut ev_b,
        |e| matches!(e, GroupEvent::State { .. }),
        "B 收 roster",
    )
    .await;
    let GroupEvent::State { group } = joined else {
        panic!("unreachable")
    };
    assert_eq!(group.group_id, g.group_id, "B 入群");
    assert!(
        c.chat
            .group
            .group_list()
            .iter()
            .any(|x| x.group_id == g.group_id),
        "C 入群（离线前在册）"
    );
    c.node.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    (a, b, c_dir, peer_c)
}

/// C 重启（同数据目录 = 同身份）并刷新 A 的地址簿；只登记不拨号，不提前触发补投。
async fn revive_c(tag: &str, a: &TestNode, c_dir: &Path, peer_c: &str) -> TestNode {
    let c2 = spawn_at(&format!("{tag}-c"), c_dir).await;
    assert_eq!(peer_str(&c2.node), peer_c, "重启身份不变");
    a.chat
        .friend_add(peer_c, "c", c2.node.listen_addrs(), None)
        .expect("刷新 c 地址");
    c2
}

/// 缺陷 a+b：C 离线期间 msg1 积压 → C 重启后紧邻的下一条 group send 命令内补投
/// 送达；发送端 acks 含全员、goutbox 出队，无双账本互斥。
#[tokio::test]
async fn backfill_lands_within_next_send_command() {
    let (a, b, c_dir, peer_c) = group_with_c_offline("grl").await;
    let peer_b = peer_str(&b.node);
    let g = a.chat.group.group_list()[0].group_id.clone();

    let r1 = a
        .chat
        .group
        .group_send(&g, ChatKind::Text, Some("第一条".into()), None, None)
        .await
        .expect("send msg1");
    assert_eq!((r1.acked, r1.recipients), (1, 2), "仅 B 实时确认");
    assert!(!r1.delivered);
    assert_eq!(backlog(&a.dir, &peer_c), 1, "C 积压 1 条 pending");

    // C 重新在线后，紧邻的下一条发送命令：返回即应已补投（「一步内」判据）
    let c2 = revive_c("grl", &a, &c_dir, &peer_c).await;
    let r2 = a
        .chat
        .group
        .group_send(&g, ChatKind::Text, Some("第二条".into()), None, None)
        .await
        .expect("send msg2");
    assert!(r2.delivered, "msg2 全员实时确认");

    // —— 此处到断言结束禁止 await：后台 flush 未被给任何机会 ——
    let hist_c = c2.chat.group.group_history(&g, None, 10).expect("c2 历史");
    let texts: Vec<_> = hist_c.iter().filter_map(|m| m.text.clone()).collect();
    assert!(
        texts.contains(&"第一条".to_string()),
        "紧邻一步收到补投 msg1：{texts:?}"
    );
    assert!(
        texts.contains(&"第二条".to_string()),
        "msg2 已送达：{texts:?}"
    );
    let hist_a = a.chat.group.group_history(&g, None, 10).expect("a 历史");
    let msg1 = hist_a
        .iter()
        .find(|m| m.text.as_deref() == Some("第一条"))
        .expect("msg1 在历史");
    assert!(
        msg1.acks.contains(&peer_b) && msg1.acks.contains(&peer_c),
        "补投送达计入 acks 且含全员：{:?}",
        msg1.acks
    );
    assert_eq!(backlog(&a.dir, &peer_c), 0, "已送达即出队，无双账本");
    assert_eq!(backlog(&a.dir, &peer_b), 0);
    cleanup(&a);
    cleanup(&b);
    cleanup(&c2);
}

/// 缺陷 c：roster 推送 failed（演练 connection lost 在 goutbox 落下的残局形状）
/// 在成员重新可达后的下一次命令内重投成功，成员 rev/群名收敛（高 rev 胜不变）。
#[tokio::test]
async fn failed_roster_entry_retries_within_next_command() {
    let (a, _b, c_dir, peer_c) = group_with_c_offline("grlr").await;
    let g = a.chat.group.group_list()[0].group_id.clone();

    // C 离线期间 rename rev1 → 推送 ConnectFailed 留 pending；置 failed 复现残局
    a.chat
        .group
        .group_rename(&g, "改名一号")
        .await
        .expect("rename rev1");
    let entry_path = a.dir.join("chat/goutbox").join(format!("{peer_c}.jsonl"));
    let raw = std::fs::read_to_string(&entry_path).expect("goutbox 文件存在");
    assert!(
        raw.contains("\"status\":\"pending\""),
        "前置：积压为 pending"
    );
    std::fs::write(
        &entry_path,
        raw.replacen("\"status\":\"pending\"", "\"status\":\"failed\"", 1),
    )
    .expect("置 failed");

    // 成员重新可达后的下一条命令：内联补投重投 failed roster
    let c2 = revive_c("grlr", &a, &c_dir, &peer_c).await;
    a.chat
        .group
        .group_send(&g, ChatKind::Text, Some("重投触发".into()), None, None)
        .await
        .expect("send 触发命令");

    // —— 同步断言，不给后台 flush 机会 ——
    assert_eq!(backlog(&a.dir, &peer_c), 0, "failed roster 重投成功出队");
    let mine = c2
        .chat
        .group
        .group_list()
        .into_iter()
        .find(|x| x.group_id == g)
        .expect("c2 在群");
    assert_eq!(mine.rev, 1, "成员 rev 收敛");
    assert_eq!(mine.name, "改名一号", "群名收敛");
    cleanup(&a);
    cleanup(&c2);
}
