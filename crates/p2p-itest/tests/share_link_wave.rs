//! SHARE 波收官 E2E（docs/design/acp-share-design.md 冻结契约全链路）：
//! ① owner 经 admin HTTP 创建分享 + 链接契约 ② guest 按链接直拨兑换
//! （token=链接 token / scope / 监狱落位）③ 对话操作 agent（stub 默认 hermetic；
//! 真 dsh 双模式 #[ignore]，不可用直写 stderr 留 SKIP 不假绿）
//! ④ 撤销级联 + 再连被拒（share-revoked）⑤ 他人重用 token（share-reuse-denied）
//! ⑥ 过期（share-expired）。真两节点 QUIC loopback；装置在 share_link_common。

#[path = "share_link_common/mod.rs"]
mod common;

use std::time::Duration;

use acp_agent::AuditEvent;
use acp_common::{Scope, ShareDenyKind};
use serde_json::{json, Value};

use common::{
    admin_call, create_share, dial_link, dial_link_fresh_slot, expect_denied, expect_ready,
    guest_node, line_within, owner_rig, read_line, sandbox_jail, send_line, skip_signal,
    wait_redeem_denied, wait_share_redeemed, DialVerdict, STEP,
};

/// 真 dsh 探测预算：启动远慢于桩。
const PROBE_SECS: u64 = 30;

/// ① admin HTTP 创建分享：token 只在创建响应出现一次（32-hex），链接契约成
/// （scheme/v1/peer/token/sid/exp），列表脱敏不留 token，错误 Bearer 拒绝。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s1_admin_creates_share_and_link_contract_holds() {
    let rig = owner_rig("share-s1", |_| {}).await;
    let created = create_share(
        &rig,
        json!({ "scope": "sandbox", "ttl_secs": 600, "note": "itest" }),
    )
    .await;
    let share_id = created["share_id"].as_str().expect("share_id").to_owned();
    let token = created["token"].as_str().expect("token").to_owned();
    assert_eq!(token.len(), 32, "token must be 32-hex: {token}");
    assert!(
        token.bytes().all(|b| b.is_ascii_hexdigit()),
        "token hex: {token}"
    );
    assert_eq!(
        created["peer"].as_str(),
        Some(rig.peer.to_string()).as_deref()
    );
    let link = created["link"].as_str().expect("link").to_owned();
    assert!(link.starts_with("dsh-acp-share://v1?peer="), "link: {link}");
    assert!(
        link.contains(&format!("&token={token}")),
        "link carries token: {link}"
    );
    assert!(
        link.contains(&format!("&sid={share_id}")),
        "link carries sid: {link}"
    );
    assert!(link.contains("&exp="), "link carries exp: {link}");
    let (status, list) = admin_call(rig.admin_addr, &rig.admin_token, "GET", "/shares", None).await;
    assert_eq!(status, 200);
    let first = &list["shares"][0];
    assert_eq!(first["share_id"].as_str(), Some(share_id).as_deref());
    assert_eq!(first["status"], "active", "fresh share is active: {first}");
    assert_eq!(first["activations"], 0);
    assert!(
        first["token"].is_null(),
        "list must not leak token: {first}"
    );
    let (status, _) = admin_call(rig.admin_addr, "wrong-token", "GET", "/shares", None).await;
    assert_eq!(status, 401, "admin HTTP must reject bad bearer");
}

/// ② guest 按分享链接直拨：token=链接 token 兑换激活，ready.scope=sandbox，
/// 子进程 cwd=监狱目录落位；台账 activations=1 且绑定 guest。
/// ③(stub 模式) 对话操作 agent：session/new 经桥到桩回声 roundtrip + share-redeemed 审计。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_guest_dials_by_link_redeems_and_talks() {
    let rig = owner_rig("share-s2", |_| {}).await;
    let created = create_share(&rig, json!({ "scope": "sandbox", "ttl_secs": 600 })).await;
    let share_id = created["share_id"].as_str().expect("share_id").to_owned();
    let token = created["token"].as_str().expect("token").to_owned();
    let link = created["link"].as_str().expect("link").to_owned();
    let guest = guest_node("share-s2-g").await;
    let (parsed, verdict) = dial_link(&guest, &link).await.expect("dial by link");
    assert_eq!(
        parsed.token, token,
        "handshake token must be the link token"
    );
    assert_eq!(
        parsed.sid.as_deref(),
        Some(share_id.as_str()),
        "sid must echo share id"
    );
    let (scope, mut stream) = expect_ready(verdict, "share dial");
    assert_eq!(
        scope,
        Scope::Sandbox,
        "ready scope must come from the share"
    );
    // 监狱目录落位：回声桩首行 = 子进程 cwd = sandbox_root/<guestPeer>
    let first = read_line(&mut stream).await.expect("child cwd line");
    let jail = sandbox_jail(&rig, &guest.peer.to_string());
    assert_eq!(
        first.trim_end(),
        jail.to_string_lossy().as_ref(),
        "child cwd must be the per-peer sandbox jail"
    );
    // 对话操作 agent：session/new 到桩回声（id==7 roundtrip）
    send_line(
        &mut stream,
        r#"{"jsonrpc":"2.0","id":7,"method":"session/new","params":{"cwd":"/tmp"}}"#,
    )
    .await;
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        let budget = deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .as_secs()
            .max(1);
        let Some(line) = line_within(&mut stream, budget).await else {
            panic!("no reply for session/new within budget");
        };
        let Ok(root) = serde_json::from_str::<Value>(line.trim_end()) else {
            continue;
        };
        if root.get("id").and_then(Value::as_i64) == Some(7) {
            assert_eq!(
                root["method"], "session/new",
                "stub must echo the request: {line}"
            );
            break;
        }
    }
    wait_share_redeemed(&rig, &share_id).await;
    let (_, list) = admin_call(rig.admin_addr, &rig.admin_token, "GET", "/shares", None).await;
    let first = &list["shares"][0];
    assert_eq!(first["activations"], 1, "one activation: {first}");
    assert_eq!(
        first["bound_peer"],
        guest.peer.to_string(),
        "bound to guest"
    );
    // 徽章优先级：exhausted（1/1）压过 bound（ShareEntry::status 判定链）
    assert_eq!(first["status"], "exhausted", "{first}");
}

/// ③ 真 dsh 双模式：子进程 = 生产命令 dsh --profile acp（ACP_E2E_REAL_DSH 可覆盖）。
/// dsh 不可用（握手被拒 / initialize 无应答）→ SKIP: 信号后结束，绝不假绿；
/// dsh 应答之后的断言失败是真实回归，照常红。
#[ignore = "真链路用例：需要本机 dsh --profile acp；以 --ignored --test-threads 1 单独跑"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s3_real_dsh_dialog_over_share_link() {
    let rig = owner_rig("share-s3", |cfg| {
        cfg.command = std::env::var("ACP_E2E_REAL_DSH")
            .unwrap_or_else(|_| "dsh --profile acp".to_owned())
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        cfg.grace_secs = 15;
        cfg.permission_timeout_secs = 25;
    })
    .await;
    let created = create_share(&rig, json!({ "scope": "sandbox", "ttl_secs": 600 })).await;
    let share_id = created["share_id"].as_str().expect("share_id").to_owned();
    let link = created["link"].as_str().expect("link").to_owned();
    let guest = guest_node("share-s3-g").await;
    let (_, verdict) = dial_link(&guest, &link).await.expect("dial by link");
    if let DialVerdict::Denied { code } = &verdict {
        skip_signal(&format!(
            "handshake denied: {code} (dsh spawn failed or policy rejected)"
        ));
        return;
    }
    let (_, mut stream) = expect_ready(verdict, "real dsh share dial");
    send_line(
        &mut stream,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}}"#,
    )
    .await;
    loop {
        let Some(line) = line_within(&mut stream, PROBE_SECS).await else {
            skip_signal("no initialize answer (dsh exited early or lacks --profile acp)");
            return;
        };
        let Ok(root) = serde_json::from_str::<Value>(line.trim_end()) else {
            continue;
        };
        if root.get("id").and_then(Value::as_i64) == Some(1) {
            assert!(
                root.get("result").is_some_and(Value::is_object),
                "initialize must be answerable on this host: {line}",
            );
            break;
        }
    }
    wait_share_redeemed(&rig, &share_id).await;
}

/// ④ 撤销：级联删 share 来源策略条目；再连被拒 share-revoked，审计双事件
/// （ShareRedeemDenied{Revoked} + ConnDenied{share-revoked}）。在线连接不主动断（设计 §3）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s4_revoke_cascades_policy_and_denies_reconnect() {
    let rig = owner_rig("share-s4", |_| {}).await;
    let created = create_share(&rig, json!({ "scope": "sandbox", "ttl_secs": 600 })).await;
    let share_id = created["share_id"].as_str().expect("share_id").to_owned();
    let link = created["link"].as_str().expect("link").to_owned();
    let guest = guest_node("share-s4-g").await;
    let (_, verdict) = dial_link(&guest, &link).await.expect("dial by link");
    let (_, stream) = expect_ready(verdict, "pre-revoke dial");
    let (status, revoked) = admin_call(
        rig.admin_addr,
        &rig.admin_token,
        "DELETE",
        &format!("/shares/{share_id}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "revoke failed: {revoked}");
    assert_eq!(
        revoked["policy_removed"], true,
        "activation must cascade policy removal: {revoked}",
    );
    assert!(
        rig.deps
            .policy
            .read()
            .unwrap()
            .lookup(&guest.peer.to_string())
            .is_none(),
        "share policy entry must be cascaded away",
    );
    drop(stream); // 释放槽位后再连必须走 share-revoked 拒绝路径
    let (_, verdict) = dial_link_fresh_slot(&guest, &link).await;
    let code = expect_denied(verdict, "post-revoke redial");
    assert_eq!(code, "share-revoked", "revoked share deny code: {code}");
    wait_redeem_denied(&rig, ShareDenyKind::Revoked).await;
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::ConnDenied { code: c, .. } if c == "share-revoked"
        )),
        "conn denied must carry share-revoked: {:?}",
        rig.audit.snapshot(),
    );
}

/// ⑤ 他人重用同 token：绑定者 != 本人 → share-reuse-denied + 审计。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s5_reuse_by_other_peer_is_denied() {
    let rig = owner_rig("share-s5", |_| {}).await;
    let created = create_share(&rig, json!({ "scope": "sandbox", "ttl_secs": 600 })).await;
    let link = created["link"].as_str().expect("link").to_owned();
    let guest = guest_node("share-s5-g").await;
    let (_, verdict) = dial_link(&guest, &link).await.expect("dial by link");
    let (_, _stream) = expect_ready(verdict, "first guest activation");
    let other = guest_node("share-s5-c").await;
    let (_, verdict) = dial_link(&other, &link).await.expect("dial by link");
    let code = expect_denied(verdict, "reuse dial");
    assert_eq!(code, "share-reuse-denied", "reuse deny code: {code}");
    wait_redeem_denied(&rig, ShareDenyKind::Reuse).await;
}

/// ⑥ 过期：ttl=1 过窗后直拨 → share-expired + 审计；列表徽章同步 expired。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s6_expired_share_is_denied() {
    let rig = owner_rig("share-s6", |_| {}).await;
    let created = create_share(&rig, json!({ "scope": "sandbox", "ttl_secs": 1 })).await;
    let link = created["link"].as_str().expect("link").to_owned();
    tokio::time::sleep(Duration::from_secs(2)).await; // 显式跨过期窗口
    let guest = guest_node("share-s6-g").await;
    let (_, verdict) = dial_link(&guest, &link).await.expect("dial by link");
    let code = expect_denied(verdict, "expired dial");
    assert_eq!(code, "share-expired", "expired deny code: {code}");
    wait_redeem_denied(&rig, ShareDenyKind::Expired).await;
    let (_, list) = admin_call(rig.admin_addr, &rig.admin_token, "GET", "/shares", None).await;
    assert_eq!(list["shares"][0]["status"], "expired", "badge: {list}");
}
