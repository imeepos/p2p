//! B4 链路夹具：出借方（RedeemFixture + ShareProxyHandler）+ 借方 B 节点 +
//! 产品侧 provider/offer/share_create 造数（W2/W3 共享事实源，不走造数捷径）。
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::llm_share_common::{
    client_for, link, proxy_request, route, spawn_node, MockUpstream, NodeFactory,
};
use crate::serve::{shares_file, RedeemFixture, ShareProxyHandler};
use llm_share_ledger::Receipt;
use llm_share_link::ledger::ShareLedger;
use llm_share_link::link::{build_link, parse_link};
use llm_share_link::redeem::PROTOCOL_ID as REDEEM_PROTOCOL;
use llm_share_link::redeem::{RedeemRequest, RedeemResponse};
use llm_share_proxy::upstream::Upstream;
use llm_share_proxy::upstream_http::HttpUpstream;
use llm_share_proxy::{ModelRoute, ProxyClient, ProxyEvent};
use p2p::{Node, PeerId, ProtocolId};
use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::offer::{self, OfferParams};
use p2p_cli::llm_share::provider::{self, Protocol, SaveParams};
use p2p_cli::llm_share::share::{share_create, ShareCreateParams};
use p2p_identity::{load_or_generate_seed, Keypair};
use tokio::time::Duration;

/// B 波夹具共用模型名与 apiKey 样例（泄露断言用非平凡串）。
pub const MODEL: &str = "gpt-4o";
pub const KEY: &str = "sk-b1b5-mock-key-0123456789abcdef";
const PERIOD: &str = "2026-09";
const STEP: Duration = Duration::from_secs(15);

type RouteFactory = Arc<dyn Fn() -> HashMap<String, ModelRoute> + Send + Sync>;

/// 真实 HttpUpstream 指向进程内 mock 的单模型路由（apiKey 用夹具 KEY）。
pub fn http_route(base: String, upstream: Arc<dyn Upstream>) -> ModelRoute {
    ModelRoute {
        base_url: base,
        api_key: KEY.to_owned(),
        upstream,
    }
}

/// 每次调用产出独立实例（ModelRoute 非 Clone，工厂化规避整表克隆）。
pub fn openai_models(mock: &crate::http_mock::MockHttpUpstream) -> HashMap<String, ModelRoute> {
    HashMap::from([(
        MODEL.to_owned(),
        http_route(
            mock.base(),
            Arc::new(HttpUpstream::new().expect("http client")),
        ),
    )])
}

/// B4 借用链路路由工厂（进程内 mock 上游）。
pub fn mock_routes(mock: Arc<MockUpstream>) -> RouteFactory {
    Arc::new(move || HashMap::from([(MODEL.to_owned(), route(mock.clone()))]))
}
/// 断言事件流以 Done 终结并取回收据。
pub fn expect_finished(events: &[ProxyEvent], tag: &str) -> Receipt {
    let Some(ProxyEvent::Finished { receipt, .. }) = events.last() else {
        panic!("{tag} 须以 Done 终结: {events:?}");
    };
    receipt.clone()
}

fn listen_tcp(node: &Node) -> String {
    node.listen_addrs()
        .into_iter()
        .find(|addr| addr.contains("/t"))
        .expect("tcp listen addr")
}

/// B4 链路夹具（字段全 pub 供测试断言）。
pub struct ShareFx {
    pub a: Arc<Node>,
    pub a_peer: PeerId,
    pub keypair: Keypair,
    pub b: Arc<Node>,
    pub b_peer: PeerId,
    pub client: ProxyClient<NodeFactory>,
    pub data_dir: String,
    pub link: String,
    pub token: String,
    pub share_id: String,
    pub expires_at: u64,
    root: PathBuf,
}

impl Drop for ShareFx {
    fn drop(&mut self) {
        self.a.shutdown();
        self.b.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl ShareFx {
    /// 出借方 + 借方 + 产品侧 provider/offer/share_create 造数一步到位。
    pub async fn setup(tag: &str, routes: crate::serve::RouteFactory) -> Self {
        let _ = p2p_log::init(Default::default());
        let root = std::env::temp_dir().join(format!("llm-e2e-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data = root.join("llm");
        std::fs::create_dir_all(&data).expect("data dir");
        let data_dir = data.display().to_string();
        let a_dir = root.join("a");
        let b_dir = root.join("b");
        std::fs::create_dir_all(&a_dir).expect("a dir");
        let keypair = load_or_generate_seed(&a_dir.join("key.seed")).expect("a seed");
        std::fs::create_dir_all(&b_dir).expect("b dir");
        let a = spawn_node(a_dir.clone()).await;
        let a_peer = a.local_peer_id();
        a.handle_protocol(Arc::new(RedeemFixture::new(&data_dir)));
        a.handle_protocol(Arc::new(ShareProxyHandler::new(
            &data_dir,
            routes,
            PERIOD,
            1_000_000,
            keypair.clone(),
        )));
        let b = spawn_node(b_dir).await;
        let b_peer = b.local_peer_id();
        link(&b, a_peer, &a).await;
        let link_text = Self::create_share(&a, &data_dir, &a_dir, &a_peer.to_string());
        let parsed = parse_link(&link_text).expect("parse own link");
        Self {
            client: client_for(b.clone()),
            b,
            b_peer,
            a,
            a_peer,
            keypair,
            data_dir,
            token: parsed.token,
            share_id: parsed.sid.expect("share_create 须携带 sid"),
            expires_at: parsed.exp.expect("share_create 须携带 exp"),
            link: link_text,
            root,
        }
    }

    /// 产品侧造数：provider + offer + share_create（token 原文只在链接一次）。
    fn create_share(a: &Arc<Node>, data_dir: &str, a_dir: &Path, peer: &str) -> String {
        provider::save(
            data_dir,
            SaveParams {
                id: Some("p1".into()),
                name: "mock".into(),
                base_url: "https://upstream.invalid/v1".into(),
                protocol: Protocol::OpenAI,
                api_key: Some("sk-provider-secret".into()),
                models: vec![MODEL.to_owned()],
                created_at: now_secs(),
            },
        )
        .expect("provider save");
        offer::publish(
            &a_dir.join("key.seed"),
            data_dir,
            &OfferParams {
                models: vec![MODEL.to_owned()],
                spare: vec![format!("{MODEL}=1000000")],
                period_ends: "2026-09-30".into(),
                max_per_req: vec![],
                rpm: 60,
                concurrency: 2,
                ttl_secs: 3600,
                retention: None,
            },
            now_secs(),
        )
        .expect("offer publish");
        let report = share_create(
            data_dir,
            ShareCreateParams {
                peer: peer.to_owned(),
                provider_id: "p1".into(),
                models: Some(vec![MODEL.to_owned()]),
                expires_at_unix: None,
                note: "b4".into(),
                addrs: vec![listen_tcp(a)],
            },
            now_secs(),
        )
        .expect("share create");
        report.link
    }

    /// 第三节点（异 peer 兑换尝试者）：root 下独立身份并已建连出借方。
    pub async fn intruder(&self, name: &str) -> Arc<Node> {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).expect("intruder dir");
        let node = spawn_node(dir).await;
        link(&node, self.a_peer, &self.a).await;
        node
    }

    /// 指定节点经链接兑换（parse → 建连 → 单帧请求 → 结构化应答）。
    pub async fn redeem_with(&self, node: &Node, link_text: &str) -> RedeemResponse {
        let parsed = parse_link(link_text).expect("link parse");
        for addr in &parsed.addrs {
            node.add_peer_address(self.a_peer, addr).expect("addr");
        }
        node.connect(self.a_peer).await.expect("connect lender");
        let protocol = ProtocolId::new(REDEEM_PROTOCOL).expect("protocol id");
        let payload = serde_json::to_vec(&RedeemRequest {
            token: parsed.token,
        })
        .expect("encode request");
        let raw = node
            .request(self.a_peer, protocol, payload, STEP)
            .await
            .expect("redeem request");
        serde_json::from_slice(&raw).expect("redeem response")
    }

    /// 借方 B 兑换当前活跃链接。
    pub async fn redeem(&self) -> RedeemResponse {
        self.redeem_with(&self.b, &self.link).await
    }

    /// 借方 B 发起一次代理借用。
    pub async fn borrow(&self, req_id: &str) -> Vec<ProxyEvent> {
        self.client
            .call(
                self.a_peer,
                &proxy_request(req_id, MODEL, 400),
                self.keypair.public(),
                STEP,
            )
            .await
            .expect("proxy call")
    }

    /// 直接过期条目（share_create 拒绝过去时点）+ 含该条目的链接。
    pub fn expired_share_link(&self, token: &str) -> String {
        let mut ledger = ShareLedger::load_or_empty(&shares_file(&self.data_dir)).expect("ledger");
        ledger.create(
            token,
            "p1",
            vec![MODEL.to_owned()],
            now_secs() - 1,
            "expired",
            now_secs(),
        );
        ledger
            .save(&shares_file(&self.data_dir))
            .expect("save ledger");
        build_link(
            &self.a_peer.to_string(),
            &[self.tcp_addr()],
            token,
            Some(now_secs() + 3600),
            None,
            Some(vec![MODEL.to_owned()]),
        )
    }

    pub fn tcp_addr(&self) -> String {
        listen_tcp(&self.a)
    }
}
