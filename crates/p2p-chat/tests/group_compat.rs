//! 好友分组（IM-T43）：模型 roundtrip、friends.json 旧记录兼容（缺 group 读回
//! 未分组）、friend_update 往返与边界（空补丁/越界组名/peer 不在簿）。

mod common;

use common::{cleanup, spawn, spawn_at};
use p2p_chat::{ChatFriend, FriendPatch};
use serde_json::json;

fn friend_json(group: serde_json::Value) -> serde_json::Value {
    json!({
        "peerId": "PeerA", "nickname": "n", "addrs": [], "note": null,
        "group": group
    })
}

/// group camelCase 逐字对齐；缺字段读回 = 未分组；空串组名可读回（历史兜底）但语义未分组。
#[test]
fn group_field_roundtrip_and_legacy_tolerance() {
    let named: ChatFriend =
        serde_json::from_value(friend_json(json!("同事"))).expect("named group");
    assert_eq!(named.group.as_deref(), Some("同事"));
    assert_eq!(
        serde_json::to_value(&named).expect("serialize")["group"],
        "同事",
        "字段名逐字为 group"
    );

    let legacy = json!({
        "peerId": "PeerA", "nickname": "n", "addrs": [], "note": null
    });
    let parsed: ChatFriend = serde_json::from_value(legacy).expect("旧记录必须可读");
    assert_eq!(parsed.group, None, "缺字段读回 None");
}

/// friends.json 旧记录（无 group）读回未分组；friend_update 后落盘含组名。
#[tokio::test]
async fn legacy_friends_json_reads_back_ungrouped_and_updates() {
    let dir = std::env::temp_dir().join(format!("p2p-chat-grp-legacy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let peer_b = p2p_identity::Keypair::generate().peer_id().to_string();
    let legacy = json!([{
        "peerId": peer_b, "nickname": "b", "addrs": [], "note": null
    }])
    .to_string();
    // 先落旧版 friends.json 再启动（Chat::new 装载时读回）：模拟旧版本数据被新版读取
    let path = dir.join("chat/friends.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, legacy).unwrap();
    let a = spawn_at("grp-legacy", &dir).await;

    let updated = a
        .chat
        .friend_update(
            &peer_b,
            &FriendPatch {
                group: Some("家人".into()),
                ..Default::default()
            },
        )
        .expect("update group");
    assert_eq!(updated.group.as_deref(), Some("家人"));

    // 落盘复核：组名随 friends_list 读回一致（磁盘 yrs 日志合并视图）；
    // friends.json 为 yrs 日志格式，迁移前旧文件已原样备份
    let list = a.chat.friends_list().expect("friends_list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].group.as_deref(), Some("家人"));
    assert_eq!(list[0].nickname, "b");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    let mut lines = on_disk.lines();
    let header: serde_json::Value =
        serde_json::from_str(lines.next().expect("格式头")).expect("格式头 JSON");
    assert_eq!(header["p2p-friends"], "yrs-v1", "yrs 日志头行: {on_disk}");
    let backup = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("friends.json.bak-yrs-")
        })
        .expect("旧 friends.json 已备份");
    assert!(
        std::fs::read_to_string(backup.path())
            .unwrap()
            .contains("peerId"),
        "备份保留旧 JSON: {}",
        std::fs::read_to_string(backup.path()).unwrap()
    );
    cleanup(&a);
}

/// friend_update 往返：改组→读回一致；空串组 = 移出分组；改昵称/备注各自生效。
#[tokio::test]
async fn friend_update_roundtrip_and_remove_group() {
    let a = spawn("grp-roundtrip").await;
    let peer_b = p2p_identity::Keypair::generate().peer_id().to_string();
    a.chat
        .friend_add_direct(&peer_b, "b", vec![], None)
        .expect("friend_add");

    let moved = a
        .chat
        .friend_update(
            &peer_b,
            &FriendPatch {
                group: Some("  同事  ".into()),
                ..Default::default()
            },
        )
        .expect("update");
    assert_eq!(moved.group.as_deref(), Some("同事"), "组名 trim");
    assert_eq!(
        a.chat.friends_list().unwrap()[0].group.as_deref(),
        Some("同事")
    );

    let removed = a
        .chat
        .friend_update(
            &peer_b,
            &FriendPatch {
                group: Some(String::new()),
                ..Default::default()
            },
        )
        .expect("remove group");
    assert_eq!(removed.group, None, "空串组 = 移出分组");

    let patched = a
        .chat
        .friend_update(
            &peer_b,
            &FriendPatch {
                nickname: Some("  b2 ".into()),
                note: Some("备注".into()),
                ..Default::default()
            },
        )
        .expect("patch nickname/note");
    assert_eq!(patched.nickname, "b2");
    assert_eq!(patched.note.as_deref(), Some("备注"));
    assert_eq!(patched.group, None, "未触及字段保持原值");
    cleanup(&a);
}

/// 边界拒绝：空补丁、33 字符组名、peer 不在簿，一律可读 Err 且不落盘。
#[tokio::test]
async fn friend_update_rejects_empty_patch_and_bad_group_and_missing_peer() {
    let a = spawn("grp-boundary").await;
    let peer_b = p2p_identity::Keypair::generate().peer_id().to_string();

    let empty = a
        .chat
        .friend_update(&peer_b, &FriendPatch::default())
        .expect_err("空补丁必须拒绝");
    assert!(empty.to_string().contains("至少提供"), "实际: {empty}");

    a.chat
        .friend_add_direct(&peer_b, "b", vec![], None)
        .expect("friend_add");
    let too_long = a
        .chat
        .friend_update(
            &peer_b,
            &FriendPatch {
                group: Some("组".repeat(33)),
                ..Default::default()
            },
        )
        .expect_err("33 字符组名必须拒绝");
    assert!(
        too_long.to_string().contains("分组名超过"),
        "实际: {too_long}"
    );
    assert_eq!(
        a.chat.friends_list().unwrap()[0].group,
        None,
        "被拒补丁不得部分落盘"
    );

    let ghost = p2p_identity::Keypair::generate().peer_id().to_string();
    let missing = a
        .chat
        .friend_update(
            &ghost,
            &FriendPatch {
                group: Some("家人".into()),
                ..Default::default()
            },
        )
        .expect_err("peer 不在簿必须拒绝");
    assert!(missing.to_string().contains("不在簿"), "实际: {missing}");
    cleanup(&a);
}
