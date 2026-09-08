//! A2A 在场发布（a2a-over-p2p-design §7.1）：rendezvous 心跳注册宣告
//! 「本节点提供 agent」+ 订阅推送心跳（卡 TTL/2）。卡片本体不经 rendezvous
//! 传输——订阅方经 /a2a/1 card 相按需取（验签后入簿）。

use std::sync::Arc;

use a2a::{CardFrame, SignedCard};
use p2p::TransportLink;
use p2p_discovery::{Discovery, RendezvousClient, RendezvousConfig};
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use crate::a2a::handler::{heartbeat_interval, A2aDeps};

/// rendezvous 注册 TTL（秒）：= 卡片 TTL（300），注册循环 20s 刷新即心跳。
const PUBLISH_TTL_SECS: u32 = a2a::TTL_DEFAULT_SECS as u32;

/// bootstrap 地址解析（ip/u端口 | ip/t端口），与 facade 同规则。
fn parse_addr(s: &str) -> Option<TransportAddr> {
    let (ip_str, tail) = s.split_once('/')?;
    let ip = ip_str.parse().ok()?;
    let mut rest = tail.chars();
    match rest.next()? {
        'u' => Some(TransportAddr::Quic {
            ip,
            port: rest.as_str().parse().ok()?,
        }),
        't' => Some(TransportAddr::Tcp {
            ip,
            port: rest.as_str().parse().ok()?,
        }),
        _ => None,
    }
}

/// 发布任务装配：bootstrap 为空 = 发布停用（本地/私有 agent 仍可经邀请直连）。
pub fn spawn_publisher(
    deps: Arc<A2aDeps>,
    bootstrap: &[String],
    listen_addrs: Vec<TransportAddr>,
) -> Option<tokio::task::JoinHandle<()>> {
    if deps.config.a2a_disabled || bootstrap.is_empty() {
        tracing::info!(target: "a2a_audit", "a2a publisher disabled (no bootstrap or disabled)");
        return None;
    }
    let mut addrs = Vec::new();
    for s in bootstrap {
        match parse_addr(s) {
            Some(addr) => addrs.push(addr),
            None => {
                tracing::warn!(target: "a2a_audit", bootstrap = %s, "bad a2a bootstrap addr, publisher disabled");
                return None;
            }
        }
    }
    let keypair = deps.keypair.clone();
    let link = match TransportLink::new(addrs, Arc::new(keypair.clone())) {
        Ok(link) => Arc::new(link),
        Err(err) => {
            tracing::warn!(target: "a2a_audit", error = %err, "a2a transport link build failed");
            return None;
        }
    };
    let mut rcfg = RendezvousConfig::new(a2a::AGENT_NAMESPACE, keypair, link);
    rcfg.addrs = listen_addrs.clone();
    rcfg.ttl_secs = PUBLISH_TTL_SECS;
    let client = Arc::new(RendezvousClient::new(rcfg));
    // 事件通道只做排空（查询发现的「其他 agent 宿主」宿主侧不消费）
    let (tx, mut rx) = mpsc::channel(64);
    let run_handle = tokio::spawn(async move {
        let _ = client.run(tx).await;
    });
    // 心跳：向已订阅 peer 周期重推当前卡全集（订阅簿 2×TTL 无心跳才除名）
    let hb_deps = deps.clone();
    let hb = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(heartbeat_interval());
        loop {
            ticker.tick().await;
            let cards = current_cards(&hb_deps);
            if cards.is_empty() {
                continue;
            }
            hb_deps.subscribers.notify(CardFrame::Push {
                v: a2a::CARD_FRAME_VERSION,
                id: 0,
                cards,
                removed: vec![],
            });
        }
    });
    Some(tokio::spawn(async move {
        while rx.recv().await.is_some() {}
        hb.abort();
        run_handle.abort();
    }))
}

/// 当前应发布卡全集：公开池只收 public（§6——私有卡仅经签名邀请投递，
/// 授权清单传空即天然过滤）。
fn current_cards(deps: &A2aDeps) -> Vec<SignedCard> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    deps.agents
        .signed_cards_for(&deps.keypair, &deps.host_peer, false, &[], now)
}

#[cfg(test)]
mod publish_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_addr_quic_tcp_and_bad() {
        assert!(parse_addr("127.0.0.1/u9000").is_some());
        assert!(parse_addr("10.0.0.1/t9000").is_some());
        assert!(parse_addr("10.0.0.1/x9000").is_none());
        assert!(parse_addr("no-addr").is_none());
        assert!(parse_addr("10.0.0.1/u").is_none());
    }

    #[test]
    fn heartbeat_is_half_card_ttl() {
        assert_eq!(
            heartbeat_interval(),
            Duration::from_secs(a2a::TTL_DEFAULT_SECS / 2)
        );
    }
}
