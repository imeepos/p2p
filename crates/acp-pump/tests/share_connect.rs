//! 分享链接直拨测试（docs/design/acp-share-design.md §2/§7）：
//! /connect-share 鉴权、成功/失败路径与 reason 词汇、握手 token=链接 token 的
//! 透传断言、reattach 票据落盘与 discovery 可见、同一链接重复导入幂等。
//! 链接解析的合法/非法用例在 src/share/link.rs 内联覆盖。

mod common;

use acp_pump::status::StatusServer;
use common::*;

/// 合法 token：32 hex（128-bit）。
fn token32() -> String {
    "a1".repeat(16)
}

fn peer_str(bytes: [u8; 32]) -> String {
    p2p::PeerId::from_bytes(bytes).to_string()
}

fn make_link(peer: &str, addr: &str) -> String {
    format!(
        "dsh-acp-share://v1?peer={peer}&addr={addr}&token={}&exp=1&sid=s-1",
        token32()
    )
}

fn body_with(link: &str) -> String {
    format!(r#"{{"link":"{link}"}}"#)
}

/// rig + status 服务（随机端口）：connect-share 契约测试的公共前缀。
async fn rig_with_status(tag: &str, mock: AgentMock) -> (Rig, std::net::SocketAddr) {
    let rig = rig(tag, mock).await;
    let status = StatusServer::start(0, rig.token.clone(), status_deps(&rig))
        .await
        .unwrap();
    (rig, status.addr)
}

/// 成功路径：ok=true 回执、agent 收到的握手 token=链接 token（透传断言）、
/// reattach 票据照常落盘（conn 与回执一致）、discovery 可见。
#[tokio::test]
async fn connect_share_success_carries_link_token_and_lands_ticket() {
    let (rig, addr) =
        rig_with_status("share-ok", AgentMock::echo_with_ticket("share-ticket-1")).await;
    let agent_addr = rig
        .agent
        .listen_addrs()
        .into_iter()
        .find(|a| a.contains("/t"))
        .expect("agent tcp listen addr");
    let link = make_link(&rig.agent_peer.to_string(), &agent_addr);
    let resp = http_post(addr, "/connect-share", Some(&rig.token), &body_with(&link)).await;
    assert!(resp.contains("\"ok\":true"), "resp={resp}");
    assert!(resp.contains("\"conn\""), "resp={resp}");

    // 透传断言：桥收到的 ClientHello.token == 链接 token。
    let hello = rig.mock.hello().expect("handshake reached mock");
    assert_eq!(hello.token.as_deref(), Some(token32().as_str()));

    // 票据照常落盘：conn 与回执一致，桥签发票据在内。
    let ticket = rig
        .tickets
        .latest_for(&rig.agent_peer.to_string())
        .expect("ticket store")
        .expect("ticket persisted for share peer");
    assert_eq!(ticket.ticket.as_deref(), Some("share-ticket-1"));
    assert!(resp.contains(&ticket.conn.to_string()), "resp={resp}");

    // discovery 可见：share peer 带地址候选进清单。
    let peers = rig.disc.snapshot();
    assert!(peers
        .iter()
        .any(|c| c.peer == rig.agent_peer.to_string() && !c.addrs.is_empty()));
    teardown(rig);
}

/// 失败路径（agent denied）：ok=false 且 reason 携带 denied 码，禁止静默。
#[tokio::test]
async fn connect_share_denied_surfaces_code() {
    let (rig, addr) = rig_with_status("share-deny", AgentMock::denying("share-revoked")).await;
    let link = make_link(&rig.agent_peer.to_string(), "/ip4/127.0.0.1/tcp/1");
    let resp = http_post(addr, "/connect-share", Some(&rig.token), &body_with(&link)).await;
    assert!(resp.contains("\"ok\":false"), "resp={resp}");
    assert!(resp.contains("share-revoked"), "resp={resp}");
    // 状态面同步可见：/status detail 携带同一 denied 词汇。
    let status_body = http_get(addr, "/status", Some(&rig.token)).await;
    assert!(
        status_body.contains("share-revoked"),
        "status={status_body}"
    );
    teardown(rig);
}

/// 失败路径（目标不可达）：ok=false 且 reason 非空（拨号失败如实上报）。
#[tokio::test]
async fn connect_share_unreachable_peer_reports_failure() {
    let (rig, addr) = rig_with_status("share-ghost", AgentMock::echo()).await;
    let ghost = peer_str([7; 32]);
    let link = make_link(&ghost, "/ip4/127.0.0.1/t9");
    let resp = http_post(addr, "/connect-share", Some(&rig.token), &body_with(&link)).await;
    assert!(resp.contains("\"ok\":false"), "resp={resp}");
    assert!(resp.contains("reason"), "resp={resp}");
    teardown(rig);
}

/// 鉴权沿既有 Bearer：无 token / 错 token 一律 401，直拨不发生。
#[tokio::test]
async fn connect_share_requires_bearer_token() {
    let (rig, addr) = rig_with_status("share-auth", AgentMock::echo()).await;
    let link = make_link(&rig.agent_peer.to_string(), "/ip4/127.0.0.1/tcp/1");
    for token in [None, Some("wrong-token")] {
        let resp = http_post(addr, "/connect-share", token, &body_with(&link)).await;
        assert!(resp.starts_with("HTTP/1.1 401"), "resp={resp}");
    }
    assert!(rig.mock.hellos().is_empty(), "no dial without auth");
    teardown(rig);
}

/// 坏入参 400：非 JSON / 缺 link 字段 / 非 share 链接，均不触发拨号。
#[tokio::test]
async fn connect_share_rejects_bad_input_with_400() {
    let (rig, addr) = rig_with_status("share-bad", AgentMock::echo()).await;
    let cases = [
        ("not-json", "invalid json"),
        (r#"{"nope":1}"#, "missing link"),
        (&body_with("https://example.com/v1?x=1")[..], "bad-link"),
    ];
    for (body, expect) in cases {
        let resp = http_post(addr, "/connect-share", Some(&rig.token), body).await;
        assert!(resp.starts_with("HTTP/1.1 400"), "body={body} resp={resp}");
        assert!(resp.contains(expect), "body={body} resp={resp}");
    }
    assert!(rig.mock.hellos().is_empty(), "no dial on bad input");
    teardown(rig);
}

/// 同一链接重复导入幂等：两次导入均成功，握手均携链接 token（激活后走策略表）。
#[tokio::test]
async fn connect_share_reimport_is_idempotent_and_replays_token() {
    let (rig, addr) = rig_with_status("share-idem", AgentMock::echo()).await;
    let agent_addr = rig
        .agent
        .listen_addrs()
        .into_iter()
        .find(|a| a.contains("/t"))
        .expect("agent tcp listen addr");
    let link = make_link(&rig.agent_peer.to_string(), &agent_addr);
    for _ in 0..2 {
        let resp = http_post(addr, "/connect-share", Some(&rig.token), &body_with(&link)).await;
        assert!(resp.contains("\"ok\":true"), "resp={resp}");
    }
    let hellos = rig.mock.hellos();
    assert_eq!(hellos.len(), 2, "both imports dialed the agent");
    for hello in hellos {
        assert_eq!(hello.token.as_deref(), Some(token32().as_str()));
    }
    teardown(rig);
}
