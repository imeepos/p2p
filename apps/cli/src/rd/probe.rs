//! rd probe：viewer 握手连通性探测（hello → hello_ack 即断）。
//! 退出码：0 = 握手接受；1 = 拒绝（含 awaiting_approval）/装配失败。

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use p2p::Node;
use p2p_identity::PeerId;

use crate::error::{CliError, CliResult};

#[derive(Args)]
pub struct ProbeArgs {
    /// 被访节点 PeerId（base58）
    #[arg(long, value_name = "PEER_ID", required = true)]
    pub peer: String,
    /// 被访节点监听地址（tcp 段，如 127.0.0.1:43987/t），缺省走地址簿/发现
    #[arg(long, value_name = "ADDR")]
    pub addr: Option<String>,
    /// 会话 id（16 hex；缺省随机生成）
    #[arg(long, value_name = "SESSION")]
    pub session: Option<String>,
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
    /// rendezvous bootstrap 地址，可多次
    #[arg(long, value_name = "ADDR")]
    pub bootstrap: Vec<String>,
}

/// 探测：握手接受即成功（JSON 行），拒绝/失败退出码 1。
pub async fn run(args: ProbeArgs) -> CliResult<()> {
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let peer = parse_peer(&args.peer)?;
    let session_id = args.session.clone().unwrap_or_else(random_session_id);
    let node = build_node(&args).await?;
    if let Some(addr) = &args.addr {
        node.add_peer_address(peer, addr)
            .map_err(|e| CliError::Runtime(format!("地址登记失败: {e}")))?;
    }
    let viewer = rd_viewer::RdViewer::new(node.clone());
    match viewer.probe(peer, session_id.clone()).await {
        Ok(sid) => {
            println!(
                "{}",
                serde_json::json!({ "kind": "probe", "ok": true, "sessionId": sid })
            );
            Ok(())
        }
        Err(e) => {
            println!(
                "{}",
                serde_json::json!({ "kind": "probe", "ok": false, "reason": e.to_string() })
            );
            Err(CliError::Runtime(format!("握手拒绝: {e}")))
        }
    }
}

/// 节点装配。
async fn build_node(args: &ProbeArgs) -> CliResult<Arc<Node>> {
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
        .map(Arc::new)
        .map_err(|e| CliError::Runtime(format!("节点启动失败: {e}")))
}

/// PeerId 解析（base58，与 tunnel connect 同规则）。
fn parse_peer(raw: &str) -> CliResult<PeerId> {
    let bytes = bs58::decode(raw.trim())
        .into_vec()
        .map_err(|e| CliError::Runtime(format!("--peer 非 base58: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CliError::Runtime(format!("--peer 长度非法（须 32 字节）: {raw}")))?;
    Ok(PeerId::from_bytes(arr))
}

/// 16 hex 随机会话 id（uuid v4 截断 16 hex，与 rd-wire 会话 id 形态一致）。
fn random_session_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_string()
}