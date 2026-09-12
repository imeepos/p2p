//! tunnel connect 运行时（gui-contract §19.3-8 对等 exempt，W-TC）：headless
//! 访侧本地回环反代前台常驻进程，对称 serve 形态。进程活 = 反代监听；
//! SIGINT/SIGTERM 优雅收口：abort 反代 accept 循环（LocalProxy 冻结面停止
//! 语义：listener 无部分状态）→ 等在途连接排空，超时显式报错非零退出（禁
//! 静默吞错）。就绪/收口以 JSON 行发 stdout（acp console 先例），日志走 stderr。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Args;
use p2p::Node;
use p2p_identity::PeerId;
use p2p_protocol::{ProtocolId, StreamFactory};
use p2p_tunnel::{LocalProxy, ProxyCtx, TunnelClient, TunnelError, TunnelIo, TunnelOpener};

use crate::error::{CliError, CliResult};

/// 在途连接排空等待上限：本地 loopback 会话毫秒级收口，10s 为宽松护栏。
const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const DRAIN_POLL: Duration = Duration::from_millis(100);

#[derive(Args)]
pub struct ConnectArgs {
    /// 被访节点 PeerId（base58）
    #[arg(long, value_name = "PEER_ID", required = true)]
    pub peer: String,
    /// 被访目标，精确 `127.0.0.1:<port>` 字面量（须在对端白名单内）
    #[arg(long, value_name = "TARGET", required = true)]
    pub target: String,
    /// 节点身份数据目录
    #[arg(long, default_value = crate::node::DEFAULT_DATA_DIR, value_name = "DIR")]
    pub data_dir: String,
    /// QUIC 监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub quic_port: u16,
    /// TCP 监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub tcp_port: u16,
    /// 关闭 mDNS 局域网发现
    #[arg(long)]
    pub no_mdns: bool,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口），可多次
    #[arg(long, value_name = "ADDR")]
    pub bootstrap: Vec<String>,
}

/// 解析后的连接意图（先校验后动作的全部产物，run 期不再碰原始字符串）。
pub struct ConnectConfig {
    pub peer: PeerId,
    pub target: String,
    pub target_port: u16,
}

/// 手写 Debug：PeerId 无 Debug 派生（p2p-identity 冻结面），以 base58 串呈现。
impl std::fmt::Debug for ConnectConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectConfig")
            .field("peer", &self.peer.to_string())
            .field("target", &self.target)
            .finish()
    }
}

/// 入参翻译：target 与 peer 先校验后动作（任一非法即 Err，不部分生效）。
pub fn build_config(args: &ConnectArgs) -> CliResult<ConnectConfig> {
    if !p2p_tunnel::is_loopback_literal_target(&args.target) {
        return Err(CliError::Runtime(format!(
            "--target 须为 127.0.0.1:<port> 字面量（端口 1-65535），当前: {}",
            args.target
        )));
    }
    let target_port = args
        .target
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse().ok())
        .ok_or_else(|| CliError::Runtime(format!("--target 端口非法: {}", args.target)))?;
    Ok(ConnectConfig {
        peer: parse_peer(&args.peer)?,
        target: args.target.clone(),
        target_port,
    })
}

/// PeerId 解析（base58，与 GUI visit 同规则）。
fn parse_peer(raw: &str) -> CliResult<PeerId> {
    let bytes = bs58::decode(raw.trim())
        .into_vec()
        .map_err(|e| CliError::Runtime(format!("--peer 非 base58: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CliError::Runtime(format!("--peer 长度非法（须 32 字节）: {raw}")))?;
    Ok(PeerId::from_bytes(arr))
}

/// 前台运行到信号；stdout 只发 JSON 行，日志走 stderr。
pub async fn run(args: ConnectArgs) -> CliResult<()> {
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let cfg = build_config(&args)?;
    let node = Arc::new(build_node(&args).await?);
    let opener = Arc::new(NodeTunnelOpener::new(Arc::clone(&node), cfg.peer));
    let proxy = LocalProxy::bind(cfg.target_port, cfg.peer.to_string(), opener)
        .await
        .map_err(|e| CliError::Runtime(format!("本地反代绑定失败: {e}")))?;
    let local_addr = proxy.local_addr();
    let ctx = Arc::clone(&proxy.ctx);
    let serve_task = tokio::spawn(proxy.serve());
    emit_ready(local_addr, &cfg.target, &cfg.peer);
    wait_signal().await;
    serve_task.abort();
    drain_active(&ctx).await.map_err(|e| {
        eprintln!("p2pctl-tunnel-connect: {e}");
        CliError::Runtime(e)
    })?;
    node.shutdown();
    emit_stopped(&ctx).await;
    Ok(())
}

/// 节点装配（serve build_node 同款缝，缺省不接公网 bootstrap）。
async fn build_node(args: &ConnectArgs) -> CliResult<Node> {
    let mut builder = Node::builder()
        .quic_port(args.quic_port)
        .tcp_port(args.tcp_port)
        .mdns(!args.no_mdns)
        .data_dir(PathBuf::from(&args.data_dir));
    if !args.bootstrap.is_empty() {
        builder = builder.bootstrap(args.bootstrap.clone());
    }
    builder
        .build()
        .await
        .map_err(|e| CliError::Runtime(format!("节点启动失败: {e}")))
}

/// 就绪行：{"kind":"ready","localAddr","target","peerId"}（peerId = 被访节点）。
fn emit_ready(local_addr: SocketAddr, target: &str, peer: &PeerId) {
    let line = serde_json::json!({
        "kind": "ready",
        "localAddr": local_addr.to_string(),
        "target": target,
        "peerId": peer.to_string(),
    });
    println!("{line}");
}

/// 收口行：访侧审计汇总。sessions = 总记录；served/rejected/broken = 已落终态
/// 计数；ended_at == 0 为存续期哨兵（tb-PROGRESS 口径），不计入三态。
async fn emit_stopped(ctx: &ProxyCtx) {
    let records = ctx.audit_snapshot().await;
    let (mut served, mut rejected, mut broken) = (0usize, 0usize, 0usize);
    for record in &records {
        if record.ended_at == 0 {
            continue;
        }
        match record.outcome {
            p2p_tunnel::TunnelAuditOutcome::Served => served += 1,
            p2p_tunnel::TunnelAuditOutcome::Rejected(_) => rejected += 1,
            p2p_tunnel::TunnelAuditOutcome::Broken(_) => broken += 1,
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "kind": "stopped",
            "sessions": records.len(),
            "served": served,
            "rejected": rejected,
            "broken": broken,
        })
    );
}

async fn wait_signal() {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("装 SIGTERM");
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("装 SIGINT");
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
}

/// 等在途连接全部收口（active_conns 归零 = 每条连接已落终态审计）。
async fn drain_active(ctx: &ProxyCtx) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + DRAIN_TIMEOUT;
    loop {
        let active = ctx.active_conns();
        if active == 0 {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "收口超时（{DRAIN_TIMEOUT:?}）：仍有 {active} 条在途连接未落终态"
            ));
        }
        tokio::time::sleep(DRAIN_POLL).await;
    }
}

/// 生产 StreamFactory：开裸流（协议 ID 首帧由 TunnelClient 唯一写入；禁
/// Node::new_stream——内嵌握手会与 TunnelClient 叠成双帧协议 ID，facade 双写
/// 缺陷 2026-09-11 裁决；与 GUI visit.rs NodeStreamFactory 同源同注释口径）。
struct NodeStreamFactory {
    node: Arc<Node>,
}

#[async_trait::async_trait]
impl StreamFactory for NodeStreamFactory {
    async fn open_stream(
        &self,
        peer: &PeerId,
        protocol: &ProtocolId,
    ) -> std::io::Result<p2p::BoxedStream> {
        self.node
            .open_raw_stream(*peer, protocol.clone())
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

/// 生产开隧道实现：TunnelClient（协议 ID/票据/ack/泵全在 p2p-tunnel 内）。
struct NodeTunnelOpener {
    client: TunnelClient<NodeStreamFactory>,
    peer: PeerId,
}

impl NodeTunnelOpener {
    fn new(node: Arc<Node>, peer: PeerId) -> Self {
        Self {
            client: TunnelClient::new(NodeStreamFactory { node }),
            peer,
        }
    }
}

#[async_trait::async_trait]
impl TunnelOpener for NodeTunnelOpener {
    async fn open(&self, uid: &str, target: &str) -> Result<TunnelIo, TunnelError> {
        // nonce 16 字节随机源取 uuid v4（apps/cli 既有依赖），hex 编码 32 字符。
        let raw = *uuid::Uuid::new_v4().as_bytes();
        let nonce = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let ticket = p2p_tunnel::TunnelTicket::new(uid, target, nonce)
            .map_err(|e| TunnelError::Io(std::io::Error::other(e)))?;
        self.client.open(self.peer, &ticket).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(peer: &str, target: &str) -> ConnectArgs {
        ConnectArgs {
            peer: peer.to_string(),
            target: target.to_string(),
            data_dir: "/tmp/td".into(),
            quic_port: 0,
            tcp_port: 0,
            no_mdns: true,
            bootstrap: vec![],
        }
    }

    fn base58_peer() -> String {
        bs58::encode([7u8; 32]).into_string()
    }

    #[test]
    fn builds_config_from_valid_target_and_peer() {
        let peer = base58_peer();
        let cfg = build_config(&args(&peer, "127.0.0.1:8014")).unwrap();
        assert_eq!(cfg.target, "127.0.0.1:8014");
        assert_eq!(cfg.target_port, 8014);
        assert_eq!(cfg.peer.to_string(), peer, "peer base58 往返一致");
    }

    #[test]
    fn rejects_non_loopback_target_without_partial_effect() {
        for bad in [
            "localhost:80",
            "127.0.0.1:0",
            "0.0.0.0:80",
            "127.0.0.1",
            "127.0.0.1:99999",
        ] {
            let err = build_config(&args(&base58_peer(), bad)).unwrap_err();
            assert!(err.to_string().contains("127.0.0.1:<port>"), "err={err}");
        }
    }

    #[test]
    fn rejects_non_base58_and_wrong_length_peer() {
        let err = build_config(&args("not base58!", "127.0.0.1:80")).unwrap_err();
        assert!(err.to_string().contains("base58"), "err={err}");
        let short = bs58::encode([1u8; 16]).into_string();
        let err = build_config(&args(&short, "127.0.0.1:80")).unwrap_err();
        assert!(err.to_string().contains("长度非法"), "err={err}");
    }
}
