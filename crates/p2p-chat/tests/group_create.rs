//! 建群校验矩阵（design im-group-design.md §5/§1）：成员 ⊆ 好友簿 / ≤32 / 不含本机 /
//! 群名 trim 1..=64；建群后 groups.json 落盘、列表可读、roster 入队。

mod common;

use common::{cleanup, peer_str, spawn};
use p2p_chat::GroupState;

fn ids(v: Vec<String>) -> Vec<String> {
    v
}

#[tokio::test]
async fn group_create_validation_matrix() {
    let a = spawn("gc-a").await;
    let b = p2p_identity::Keypair::generate().peer_id().to_string();
    let c = p2p_identity::Keypair::generate().peer_id().to_string();
    let own = peer_str(&a.node);
    a.chat.friend_add(&b, "b", vec![], None).expect("add b");
    a.chat.friend_add(&c, "c", vec![], None).expect("add c");

    // 群名：trim 生效；空/纯空白拒绝；65 字符拒绝；64 字符允许
    assert!(a
        .chat
        .group
        .group_create("   ", &ids(vec![b.clone()]))
        .await
        .is_err());
    let g = a
        .chat
        .group
        .group_create("  我的小组 ", std::slice::from_ref(&b))
        .await
        .expect("create");
    assert_eq!(g.name, "我的小组");
    assert_eq!(g.owner, own, "owner = 本机");
    assert_eq!(g.members, vec![own.clone(), b.clone()], "members 含 owner");
    assert_eq!(g.rev, 0);
    assert_eq!(g.state, GroupState::Active);
    let long = a
        .chat
        .group
        .group_create(&"n".repeat(65), &ids(vec![b.clone()]))
        .await
        .expect_err("65 字符群名必须拒绝");
    assert!(long.to_string().contains("群名"), "err: {long}");
    assert!(
        a.chat
            .group
            .group_create(&"n".repeat(64), &[])
            .await
            .is_ok(),
        "64 字符允许"
    );

    // 成员 ⊆ 好友簿；非好友拒绝
    let stranger = p2p_identity::Keypair::generate().peer_id().to_string();
    let err = a
        .chat
        .group
        .group_create("x", &ids(vec![stranger.clone()]))
        .await
        .expect_err("非好友必须拒绝");
    assert!(err.to_string().contains("好友簿"), "err: {err}");

    // 成员不含本机
    let err = a
        .chat
        .group
        .group_create("x", &ids(vec![own.clone()]))
        .await
        .expect_err("含本机必须拒绝");
    assert!(
        err.to_string().contains("自己") || err.to_string().contains("本机"),
        "err: {err}"
    );

    // 成员上限：31 位好友 + 本机 = 32 允许；32 位好友 + 本机 = 33 拒绝
    let mut many = Vec::new();
    for i in 0..32 {
        let p = p2p_identity::Keypair::generate().peer_id().to_string();
        a.chat
            .friend_add(&p, &format!("f{i}"), vec![], None)
            .expect("add");
        many.push(p);
    }
    let g31 = a
        .chat
        .group
        .group_create("满员群", &ids(many[..31].to_vec()))
        .await
        .expect("31 成员 + owner = 32 允许");
    assert_eq!(g31.members.len(), 32);
    let err = a
        .chat
        .group
        .group_create("超员群", &ids(many.clone()))
        .await
        .expect_err("33 人必须拒绝");
    assert!(err.to_string().contains("上限"), "err: {err}");

    // 重复入参幂等去重
    let g2 = a
        .chat
        .group
        .group_create("去重群", &ids(vec![b.clone(), b.clone(), c.clone()]))
        .await
        .expect("dup 入参幂等");
    assert_eq!(
        g2.members,
        vec![own.clone(), b.clone(), c.clone()],
        "去重后顺序稳定"
    );

    cleanup(&a);
}

/// 建群落盘与列表：groups.json 原子写（camelCase）、group_list 全量可读。
#[tokio::test]
async fn group_create_persists_and_lists() {
    let a = spawn("gc-persist").await;
    let b = p2p_identity::Keypair::generate().peer_id().to_string();
    a.chat.friend_add(&b, "b", vec![], None).expect("add b");
    let g = a
        .chat
        .group
        .group_create("落盘群", &ids(vec![b.clone()]))
        .await
        .expect("create");

    let raw = std::fs::read_to_string(a.dir.join("chat/groups.json")).expect("groups.json");
    assert!(
        raw.contains("\"groupId\"") && raw.contains("\"落盘群\""),
        "camelCase 落盘: {raw}"
    );
    let list = a.chat.group.group_list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].group_id, g.group_id);
    assert_eq!(list[0].state, GroupState::Active);
    cleanup(&a);
}
