//! share_redeem 借方编排集成（W3）：真 facade Node TCP 互联；出借方挂
//! /llm-share/redeem/1 剧本桩与 /llm-share/offer/1 快照桩，覆盖兑换成功快照、
//! 结构化拒绝码透出（业务结果非 Err）、协议违例显式 Err、链接参数错误显式 Err。

//! RS-II 轨基准注记：桩经 handle_inbound 接线（swarm 已消费协议 ID 首帧）。
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use llm_share_link::link::build_link;
use llm_share_link::redeem::{
    RedeemCode, RedeemRequest, RedeemResponse, PROTOCOL_ID as REDEEM_PROTOCOL,
};
use llm_share_offer::{Offer, RateLimit, SignedOffer, PROTOCOL_ID as OFFER_PROTOCOL};
use p2p::{BoxedStream, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_identity::{load_or_generate_seed, Keypair};
use p2p_protocol::{read_chunked, read_frame, write_chunked, write_frame};

use p2p_cli::llm_share::share_redeem::{self, RedeemParams, RedeemStatus};

const MODEL: &str = "gpt-4o";

/// 兑换剧本桩：逐次弹出应答；弹尽后统一回 invalid（缺省拒绝不悬挂）。
struct RedeemStub {
    script: Mutex<Vec<RedeemResponse>>,
}

#[async_trait]
impl ProtocolHandler for RedeemStub {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(REDEEM_PROTOCOL).expect("redeem protocol id")
    }

    async fn handle_inbound(&self, _peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let raw = read_frame(&mut stream).await?;
        let req: RedeemRequest = serde_json::from_slice(&raw)?;
        assert_eq!(req.token.len(), 32, "借方须按链接原文携带 token");
        let reply = self
            .script
            .lock()
            .expect("script lock")
            .pop()
            .unwrap_or_else(|| RedeemResponse::deny(RedeemCode::Invalid));
        write_frame(&mut stream, &serde_json::to_vec(&reply)?).await
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        self.handle_inbound(PeerId::from_bytes([0u8; 32]), stream)
            .await
    }
}

/// offer 快照桩：dispatch 路径首帧即 "get"（协议 ID 已被 swarm 消费）。
struct OfferStub {
    signed: Arc<SignedOffer>,
}

#[async_trait]
impl ProtocolHandler for OfferStub {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(OFFER_PROTOCOL).expect("offer protocol id")
    }

    async fn handle_inbound(&self, _peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let get = read_chunked(&mut stream).await?;
        assert_eq!(get, b"get");
        write_chunked(&mut stream, &serde_json::to_vec(self.signed.as_ref())?).await
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        self.handle_inbound(PeerId::from_bytes([0u8; 32]), stream)
            .await
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("w3_redeem_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn spawn_lender(
    tag: &str,
    script: Vec<RedeemResponse>,
) -> (Arc<Node>, String, Keypair, TempDir) {
    let dir = TempDir::new(tag);
    let keypair = load_or_generate_seed(&dir.0.join("key.seed")).expect("lender seed");
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(dir.0.clone())
            .build()
            .await
            .expect("node"),
    );
    let offer = Offer {
        peer: keypair.peer_id().to_string(),
        models: vec![MODEL.to_owned()],
        spare: [(MODEL.to_owned(), 150u64)].into_iter().collect(),
        period_ends: "2026-09-30".to_owned(),
        max_per_req: Default::default(),
        rate_limit: RateLimit {
            rpm: 10,
            concurrency: 2,
        },
        ttl_secs: 3600,
        retention: "none".to_owned(),
    };
    let signed = SignedOffer::sign(&offer, &keypair, p2p_cli::llm_share::now_secs()).expect("sign");
    node.handle_protocol(Arc::new(RedeemStub {
        script: Mutex::new(script),
    }));
    node.handle_protocol(Arc::new(OfferStub {
        signed: Arc::new(signed),
    }));
    let addr = node
        .listen_addrs()
        .into_iter()
        .find(|a| a.contains("/t"))
        .expect("tcp listen addr");
    (node, addr, keypair, dir)
}

fn params(link: String, dir: &TempDir) -> RedeemParams {
    RedeemParams {
        link,
        bootstrap: Vec::new(),
        node_dir: dir.0.display().to_string(),
        timeout_secs: 15,
        discover_secs: 5,
    }
}

#[tokio::test]
async fn redeem_success_returns_verified_offer_snapshot() {
    let _ = p2p_log::init(Default::default());
    let (lender, addr, keypair, lender_dir) = spawn_lender("ok", vec![RedeemResponse::ok()]).await;
    let borrower_dir = TempDir::new("ok-b");
    let token = "0123456789abcdef0123456789abcdef";
    let link = build_link(
        &keypair.peer_id().to_string(),
        std::slice::from_ref(&addr),
        token,
        Some(p2p_cli::llm_share::now_secs() + 3600),
        Some("sid-1".to_owned()),
        Some(vec![MODEL.to_owned()]),
    );
    let outcome = share_redeem::run(&params(link, &borrower_dir))
        .await
        .expect("redeemed");
    assert_eq!(outcome.status, RedeemStatus::Redeemed);
    let offer = outcome.offer.expect("offer snapshot");
    assert_eq!(offer.peer, keypair.peer_id().to_string());
    assert_eq!(offer.models, vec![MODEL.to_owned()]);
    assert_eq!(offer.spare[MODEL], 150);
    assert_eq!(offer.period_ends, "2026-09-30");
    assert_eq!(
        outcome.owner.as_deref(),
        Some(keypair.peer_id().to_string().as_str())
    );
    assert_eq!(outcome.share_id.as_deref(), Some("sid-1"));
    drop(lender);
    drop(lender_dir);
}

#[tokio::test]
async fn redeem_structured_denial_passes_through_as_business_result() {
    let _ = p2p_log::init(Default::default());
    let (lender, addr, keypair, lender_dir) =
        spawn_lender("deny", vec![RedeemResponse::deny(RedeemCode::ShareRevoked)]).await;
    let borrower_dir = TempDir::new("deny-b");
    let link = build_link(
        &keypair.peer_id().to_string(),
        std::slice::from_ref(&addr),
        "0123456789abcdef0123456789abcdef",
        Some(p2p_cli::llm_share::now_secs() + 3600),
        None,
        None,
    );
    let outcome = share_redeem::run(&params(link, &borrower_dir))
        .await
        .expect("report");
    assert_eq!(outcome.status, RedeemStatus::Rejected);
    assert_eq!(outcome.code.as_deref(), Some("share-revoked"));
    assert!(outcome.offer.is_none());
    assert!(outcome.owner.is_none());
    drop(lender);
    drop(lender_dir);
}

#[tokio::test]
async fn redeem_protocol_violation_is_explicit_error() {
    let _ = p2p_log::init(Default::default());
    let (lender, addr, keypair, lender_dir) = spawn_lender(
        "bare",
        vec![RedeemResponse {
            ok: false,
            code: None,
        }],
    )
    .await;
    let borrower_dir = TempDir::new("bare-b");
    let link = build_link(
        &keypair.peer_id().to_string(),
        std::slice::from_ref(&addr),
        "0123456789abcdef0123456789abcdef",
        None,
        None,
        None,
    );
    let err = share_redeem::run(&params(link, &borrower_dir))
        .await
        .expect_err("protocol violation");
    assert!(err.contains("拒绝码"), "err: {err}");
    drop(lender);
    drop(lender_dir);
}

#[tokio::test]
async fn redeem_bad_link_is_parameter_error() {
    let _ = p2p_log::init(Default::default());
    let borrower_dir = TempDir::new("badlink");
    let outcome = share_redeem::run(&params(
        "https://example.invalid/x".to_owned(),
        &borrower_dir,
    ))
    .await;
    let err = outcome.expect_err("bad scheme");
    assert!(err.contains("scheme"), "err: {err}");
}
