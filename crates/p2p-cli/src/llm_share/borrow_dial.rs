//! borrow 拨号装配（进程内一次性 Node，F11/PR6）：直连地址优先，缺省走
//! rendezvous 查号（bootstrap 由装配方从节点配置注入）；/llm-share/offer/1 取回
//! 信封（帧约定：new_stream 已写协议 ID 首帧，随后 "get" 请求帧，应答为
//! SignedOffer JSON）；proxy 流经 StreamFactory 适配交给 llm-share-proxy::ProxyClient
//! （工厂给裸流 open_raw_stream，协议 ID 帧由 proxy 层 open_with_protocol 写）。

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use llm_share_offer::{SignedOffer, PROTOCOL_ID as OFFER_PROTOCOL};
use p2p::Node;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_chunked, write_chunked, ProtocolId, StreamFactory};

use super::borrow::BorrowParams;

/// 建连与 offer 取回的单步上限：本地 loopback 毫秒级，跨网拨号含降级链。
const LINK_TIMEOUT: Duration = Duration::from_secs(30);

/// 查号循环轮询间隔。
const DISCOVER_POLL: Duration = Duration::from_millis(500);

/// 一次性借方节点：随机端口、mdns 关、bootstrap 可选接线。
pub async fn build_node(node_dir: &str, bootstrap: &[String]) -> Result<Arc<Node>, String> {
    let mut builder = Node::builder()
        .mdns(false)
        .data_dir(PathBuf::from(node_dir));
    if !bootstrap.is_empty() {
        builder = builder.bootstrap(bootstrap.to_vec());
    }
    let node = builder
        .build()
        .await
        .map_err(|e| format!("节点装配失败（data-dir={node_dir}）: {e}"))?;
    Ok(Arc::new(node))
}

/// 进程退出前确保节点关停（错误路径同样断连，不残留监听）。
pub struct NodeShutdown(Arc<Node>);

impl NodeShutdown {
    pub fn new(node: Arc<Node>) -> Self {
        Self(node)
    }

    pub fn node(&self) -> &Node {
        &self.0
    }

    pub fn raw(&self) -> Arc<Node> {
        self.0.clone()
    }
}

impl Drop for NodeShutdown {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

/// 连接出借方：--addr 直连登记后建连；缺省先 rendezvous 查号再登记建连。
pub async fn connect_lender(
    node: &Node,
    lender: PeerId,
    params: &BorrowParams,
) -> Result<(), String> {
    match &params.addr {
        Some(addr) => {
            node.add_peer_address(lender, addr)
                .map_err(|e| format!("CONNECT-FAIL: 出借方地址非法（{addr}）: {e}"))?;
        }
        None => {
            let addr = discover(node, &params.lender, params.discover_secs).await?;
            node.add_peer_address(lender, &addr)
                .map_err(|e| format!("CONNECT-FAIL: 查号地址非法（{addr}）: {e}"))?;
        }
    }
    tokio::time::timeout(LINK_TIMEOUT, node.connect(lender))
        .await
        .map_err(|_| "CONNECT-FAIL: 建连超时".to_owned())?
        .map_err(|e| format!("CONNECT-FAIL: {e}"))
}

/// rendezvous 查号：有界轮询直至拿到地址；bootstrap 未接线属确定性失败，
/// 立即显式报错不打满等待窗。
pub async fn discover(node: &Node, lender: &str, wait_secs: u64) -> Result<String, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_secs);
    loop {
        match node.query_peer(lender).await {
            Ok(addrs) if !addrs.is_empty() => return Ok(addrs[0].clone()),
            Err(e) if e.to_string().contains("not wired") => {
                return Err(
                    "DISCOVER-FAIL: bootstrap 未接线（rendezvous 不可用）；用 --addr 直连或先配置 bootstrap"
                        .into(),
                );
            }
            _ if tokio::time::Instant::now() >= deadline => {
                return Err(
                    "DISCOVER-FAIL: rendezvous 查号超时（出借方未注册或已过期）；可用 --addr 直连"
                        .into(),
                );
            }
            _ => tokio::time::sleep(DISCOVER_POLL).await,
        }
    }
}

/// 拉取并解析能力声明信封（验签在调用方 OfferBook::insert 完成）。
pub async fn fetch_offer(node: &Node, peer: PeerId) -> Result<SignedOffer, String> {
    let id = ProtocolId::new(OFFER_PROTOCOL).map_err(|e| format!("offer 协议 ID 非法: {e}"))?;
    let mut stream = node
        .new_stream(peer, id)
        .await
        .map_err(|e| format!("OFFER-FAIL: 开流失败: {e}"))?;
    tokio::time::timeout(LINK_TIMEOUT, exchange_offer(&mut stream))
        .await
        .map_err(|_| "OFFER-FAIL: 应答超时".to_owned())?
}

async fn exchange_offer(stream: &mut BoxedStream) -> Result<SignedOffer, String> {
    write_chunked(stream, b"get")
        .await
        .map_err(|e| format!("OFFER-FAIL: 请求帧写出失败: {e}"))?;
    let payload = read_chunked(stream)
        .await
        .map_err(|e| format!("OFFER-FAIL: 应答读取失败: {e}"))?;
    serde_json::from_slice(&payload).map_err(|e| format!("OFFER-FAIL: 应答解析失败: {e}"))
}

/// 借方侧拨号工厂：裸流交 llm-share-proxy 层握手（open_raw_stream；工厂禁包
/// new_stream，否则两层协议 ID 帧叠加，严格对端翻车——2026-09-11 装配 MUST）。
pub struct NodeFactory {
    pub node: Arc<Node>,
}

#[async_trait]
impl StreamFactory for NodeFactory {
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> io::Result<BoxedStream> {
        self.node
            .open_raw_stream(*peer, protocol.clone())
            .await
            .map_err(|e| io::Error::other(e.to_string()))
    }
}
