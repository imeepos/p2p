//! vdrive serve 运行时：本地目录监狱 → /vdrive/fs/1 全网可见。
//! stdout 只发 JSON 行（ready/stopped），日志走 stderr。

use std::sync::Arc;

use clap::Args;
use serde_json::json;

use crate::error::CliResult;
use crate::vdrive::common::{build_node, emit, wait_signal, NodeArgs};

#[derive(Args)]
pub struct ServeArgs {
    /// 对外暴露的本地根目录（必须已存在）
    #[arg(long = "root", value_name = "DIR", required = true)]
    pub root: String,
    /// 只读模式：拒绝全部写类操作（mkdir/unlink/write/...）
    #[arg(long)]
    pub read_only: bool,
    #[command(flatten)]
    pub node: NodeArgs,
}

/// 前台运行到信号。
pub async fn run(args: ServeArgs) -> CliResult<()> {
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let fs: Arc<dyn p2p_vdrive::FsBackend> =
        Arc::new(p2p_vdrive::LocalFs::open(&args.root).await.map_err(|e| {
            crate::error::CliError::Runtime(format!("--root 不可用: {e}"))
        })?);
    let node = build_node(&args.node).await?;
    if args.read_only {
        p2p_vdrive::serve_with_policy(
            &node,
            fs,
            Arc::new(ReadOnlyPolicy),
        )
        .map_err(|e| crate::error::CliError::Runtime(format!("vdrive 装配失败: {e}")))?;
    } else {
        p2p_vdrive::serve(&node, fs)
            .map_err(|e| crate::error::CliError::Runtime(format!("vdrive 装配失败: {e}")))?;
    }
    emit(json!({
        "kind": "ready",
        "role": "serve",
        "peerId": node.local_peer_id().to_string(),
        "listenAddrs": node.listen_addrs(),
        "root": args.root,
        "readOnly": args.read_only,
    }));
    wait_signal().await;
    node.shutdown();
    emit(json!({"kind": "stopped", "role": "serve"}));
    Ok(())
}

/// 只读策略：写类操作一律拒绝（策略路径显式拒绝并留日志，非静默）。
struct ReadOnlyPolicy;

#[async_trait::async_trait]
impl p2p_vdrive::AccessPolicy for ReadOnlyPolicy {
    async fn allow(&self, _peer: &p2p_identity::PeerId, class: p2p_vdrive::OpClass) -> bool {
        match class {
            p2p_vdrive::OpClass::Read => true,
            p2p_vdrive::OpClass::Write => false,
        }
    }
}
