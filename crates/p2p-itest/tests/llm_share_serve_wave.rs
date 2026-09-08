//! B 波 serve 侧用例：B6 双借方并发归属（handle_inbound 直采流身份，无 gate
//! 串线）+ B7 重启幂等（重建 settled 索引后同 req_id 重放被 DuplicateReqId 拒）。

mod llm_share_common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[path = "llm_share_b_common/persist.rs"]
mod persist;

use llm_share_common::{
    call, client_for, link, proxy_request, rig_custom, route, spawn_node, usage_chunk,
    MockUpstream, NodeFactory, Script, STEP,
};
use llm_share_ledger::Receipt;
use llm_share_proxy::{ErrorCode, LenderProxy, ProxyClient, ProxyConfig, ProxyEvent, ProxyRequest};
use p2p::{Node, PeerId};
use p2p_identity::{load_or_generate_seed, Keypair};
use persist::PersistProxyHandler;

/// B6：双借方并发拨号，各自收据/流水按流身份归属（handle_inbound 直采认证
/// PeerId，无 gate 串线）；C 的身份装配期预派生并直入白名单。
#[tokio::test]
async fn b6_concurrent_borrowers_attributed_by_stream_identity() {
    let mock = MockUpstream::new(vec![
        Script::Canned(vec![usage_chunk(20, 8)]),
        Script::Canned(vec![usage_chunk(10, 5)]),
    ]);
    let c_dir = std::env::temp_dir().join(format!("llm-e2e-b6-c-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c_dir);
    std::fs::create_dir_all(&c_dir).expect("c dir");
    let c_keypair = p2p_identity::load_or_generate_seed(&c_dir.join("key.seed")).expect("c seed");
    let c_peer = c_keypair.peer_id();
    let models = HashMap::from([("gpt-4o".to_string(), route(mock.clone()))]);
    let rig = rig_custom("b6", models, vec![c_peer.to_string()], 1_000_000).await;
    let c = spawn_node(c_dir.clone()).await;
    assert_eq!(c.local_peer_id(), c_peer, "C 身份须与预派生种子一致");
    link(&c, rig.a_peer, &rig.a).await;
    let c_client = client_for(c.clone());
    let b_req = proxy_request("req-b6-b", "gpt-4o", 400);
    let c_req = proxy_request("req-b6-c", "gpt-4o", 400);
    let (b_events, c_events) = tokio::join!(call(&rig, &b_req), async {
        c_client
            .call(rig.a_peer, &c_req, rig.keypair.public(), STEP)
            .await
            .expect("c call")
    },);
    assert_eq!(mock.calls(), 2, "双借方各触发一次上游");
    let b_receipt = expect_finished(&b_events, "B");
    let c_receipt = expect_finished(&c_events, "C");
    assert_eq!(b_receipt.req_id, "req-b6-b");
    assert_eq!(b_receipt.borrower, rig.b_peer.to_string(), "B 收据归属 B");
    assert_eq!(c_receipt.req_id, "req-b6-c");
    assert_eq!(c_receipt.borrower, c_peer.to_string(), "C 收据归属 C");
    let ledger = rig.proxy.ledger().await;
    // 归属独立于剧本弹出顺序：各自净差与本人收据 usage 一致，出借方为合计。
    let b_total = (b_receipt.usage.input + b_receipt.usage.output) as i64;
    let c_total = (c_receipt.usage.input + c_receipt.usage.output) as i64;
    assert_eq!(b_total + c_total, 43, "两份剧本 usage 合计");
    assert_eq!(ledger.net(&rig.b_peer.to_string(), "2026-09"), -b_total);
    assert_eq!(ledger.net(&c_peer.to_string(), "2026-09"), -c_total);
    assert_eq!(
        ledger.net(&rig.a_peer.to_string(), "2026-09"),
        b_total + c_total
    );
    c.shutdown();
    let _ = std::fs::remove_dir_all(&c_dir);
}

/// B7 装配：出借方 + 借方节点对，handler 由测试分阶段装配（模拟重启换装）。
struct RestartRig {
    a: Arc<Node>,
    a_peer: PeerId,
    keypair: Keypair,
    client: ProxyClient<NodeFactory>,
    b: Arc<Node>,
    b_peer: PeerId,
    data_dir: String,
    root: PathBuf,
}

impl Drop for RestartRig {
    fn drop(&mut self) {
        self.a.shutdown();
        self.b.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn restart_rig(tag: &str) -> RestartRig {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("llm-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).expect("a dir");
    let keypair = load_or_generate_seed(&a_dir.join("key.seed")).expect("a seed");
    std::fs::create_dir_all(&b_dir).expect("b dir");
    let a = spawn_node(a_dir.clone()).await;
    let a_peer = a.local_peer_id();
    let b = spawn_node(b_dir).await;
    let b_peer = b.local_peer_id();
    link(&b, a_peer, &a).await;
    RestartRig {
        a,
        a_peer,
        client: client_for(b.clone()),
        keypair,
        b,
        b_peer,
        data_dir: a_dir.display().to_string(),
        root,
    }
}

/// 装配一个 serve 实例：全新 LenderProxy（空内存索引）+ persisted 门禁换装。
fn serve_proxy(rig: &RestartRig, mock: Arc<MockUpstream>) {
    let models = HashMap::from([("gpt-4o".to_string(), route(mock))]);
    let cfg = ProxyConfig {
        lender_id: rig.a_peer.to_string(),
        period: "2026-09".into(),
        net_limit: 1_000_000,
        max_concurrent: 4,
        allowlist: [rig.b_peer.to_string()].into_iter().collect(),
        models,
    };
    let proxy = Arc::new(LenderProxy::new(cfg, rig.keypair.clone()));
    rig.a.handle_protocol(Arc::new(PersistProxyHandler::new(
        proxy,
        &rig.data_dir,
        &rig.keypair,
    )));
}

async fn dial(rig: &RestartRig, req: &ProxyRequest) -> Vec<ProxyEvent> {
    rig.client
        .call(rig.a_peer, req, rig.keypair.public(), STEP)
        .await
        .expect("proxy call")
}

fn expect_finished(events: &[ProxyEvent], tag: &str) -> Receipt {
    let Some(ProxyEvent::Finished { receipt, .. }) = events.last() else {
        panic!("{tag} 须以 Done 终结: {events:?}");
    };
    receipt.clone()
}

/// B7：换装的 serve 实例（新 LenderProxy 内存索引为空）从 ledger.json 重建
/// settled 索引，同 req_id 重放被 DuplicateReqId 拒并回传原收据，上游零增量。
#[tokio::test]
async fn b7_restart_rebuilt_settled_index_rejects_replay() {
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(10, 5)])]);
    let rig = restart_rig("b7").await;
    serve_proxy(&rig, mock.clone());
    let req = proxy_request("req-b7", "gpt-4o", 400);
    let first = dial(&rig, &req).await;
    let receipt = expect_finished(&first, "首笔");
    assert_eq!(mock.calls(), 1, "首笔恰一次上游");
    let persisted =
        p2p_cli::llm_share::ledger::load_or_empty(&p2p_cli::llm_share::ledger::path(&rig.data_dir))
            .expect("ledger 可读");
    assert_eq!(persisted.receipts.len(), 1, "收据已持久化");
    assert_eq!(persisted.receipts[0].req_id, "req-b7");
    // 重启：handler 换装（内存索引为空），重建 settled 后重放必拒。
    serve_proxy(&rig, mock.clone());
    let second = dial(&rig, &req).await;
    let Some(ProxyEvent::Rejected {
        code: ErrorCode::DuplicateReqId,
        receipt: Some(replayed),
        ..
    }) = second.last()
    else {
        panic!("重放须被重建索引拒绝: {second:?}");
    };
    assert_eq!(replayed, &receipt, "重放回传原收据");
    assert_eq!(mock.calls(), 1, "重放不得再打上游");
}
