//! vdrive mount 运行时：远端 Peer 目录 → 本机回环 WebDAV 桥；macOS 可
//! --mount 自动 mount_webdav 到 /Volumes/<name>，退出时自动卸载。

use std::sync::Arc;

use clap::Args;
use p2p_identity::PeerId;
use serde_json::json;
use tokio::sync::watch;

use crate::error::{CliError, CliResult};
use crate::vdrive::common::{build_node, emit, parse_peer, wait_signal, NodeArgs};

#[derive(Args)]
pub struct MountArgs {
    /// 被挂端 PeerId（base58）
    #[arg(long, value_name = "PEER_ID", required = true)]
    pub peer: String,
    /// 本机桥监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub port: u16,
    /// 自动挂载（macOS：mount_webdav → /Volumes/<name>；退出时自动卸载）
    #[arg(long)]
    pub mount: bool,
    /// 卷名 / 挂载点目录名
    #[arg(long, default_value = "vdrive", value_name = "NAME")]
    pub name: String,
    #[command(flatten)]
    pub node: NodeArgs,
}

/// 解析后的挂载意图（先校验后动作）。
pub struct MountPlan {
    pub peer: PeerId,
    pub port: u16,
    pub auto_mount: bool,
    pub name: String,
}

/// 卷名校验：非空且不含路径分隔符（挂载点由本函数面拼装）。
pub fn validate_name(name: &str) -> CliResult<String> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(CliError::Runtime(format!("--name 非法: {name}")));
    }
    Ok(name.to_string())
}

pub fn build_plan(args: &MountArgs) -> CliResult<MountPlan> {
    Ok(MountPlan {
        peer: parse_peer(&args.peer)?,
        port: args.port,
        auto_mount: args.mount,
        name: validate_name(&args.name)?,
    })
}

/// 前台运行到信号；桥就绪后 stdout 发 ready 行（url 与挂载命令提示）。
pub async fn run(args: MountArgs) -> CliResult<()> {
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let plan = build_plan(&args)?;
    let node = Arc::new(build_node(&args.node).await?);
    let client: Arc<dyn p2p_vdrive::FsBackend> = Arc::new(
        p2p_vdrive::VDriveClient::new(Arc::clone(&node), plan.peer)
            .map_err(|e| CliError::Runtime(format!("vdrive 客户端装配失败: {e}")))?,
    );
    // 连通性预检：桥就绪前先确认远端可达（statfs 5s 上限，显式报错）。
    tokio::time::timeout(std::time::Duration::from_secs(5), client.statfs())
        .await
        .map_err(|_| CliError::Runtime("对端 statfs 超时（5s）：节点不可达？".into()))?
        .map_err(|e| CliError::Runtime(format!("对端不可用: {e}")))?;
    let bridge = p2p_vdrive::MountBridge::bind(p2p_vdrive::MountConfig {
        port: plan.port,
        ..p2p_vdrive::MountConfig::default()
    })
    .await
    .map_err(|e| CliError::Runtime(format!("桥端口绑定失败: {e}")))?;
    let url = format!("http://{}", bridge.addr);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let serve_task = tokio::spawn(bridge.run(client, shutdown_rx));
    emit(json!({
        "kind": "ready",
        "role": "mount",
        "url": url,
        "peerId": plan.peer.to_string(),
        "hint": format!("mount_webdav {url}/ /Volumes/{}", plan.name),
    }));
    let mountpoint = if plan.auto_mount {
        mount_volume(&url, &plan.name).await
    } else {
        None
    };
    wait_signal().await;
    shutdown_tx.send(true).ok();
    if let Some(mp) = &mountpoint {
        unmount_volume(mp).await;
    }
    let _ = serve_task.await;
    node.shutdown();
    emit(json!({"kind": "stopped", "role": "mount"}));
    Ok(())
}

#[cfg(target_os = "macos")]
async fn mount_volume(url: &str, name: &str) -> Option<String> {
    let mountpoint = format!("/Volumes/{name}");
    if let Err(e) = tokio::fs::create_dir_all(&mountpoint).await {
        emit(json!({"kind": "mount-failed", "reason": format!("挂载点创建失败: {e}")}));
        return None;
    }
    let out = tokio::process::Command::new("mount_webdav")
        .args([format!("{url}/"), mountpoint.clone()])
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            emit(json!({"kind": "mounted", "mountpoint": mountpoint}));
            Some(mountpoint)
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            emit(json!({"kind": "mount-failed", "reason": stderr, "hint": url}));
            None
        }
        Err(e) => {
            emit(json!({"kind": "mount-failed", "reason": format!("mount_webdav 启动失败: {e}")}));
            None
        }
    }
}

#[cfg(target_os = "macos")]
async fn unmount_volume(mountpoint: &str) {
    let out = tokio::process::Command::new("diskutil")
        .args(["unmount", mountpoint])
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            emit(json!({"kind": "unmounted", "mountpoint": mountpoint}));
        }
        other => {
            let detail = other
                .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
                .unwrap_or_else(|e| e.to_string());
            emit(json!({"kind": "unmount-failed", "mountpoint": mountpoint, "reason": detail}));
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn mount_volume(_url: &str, _name: &str) -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
async fn unmount_volume(_mountpoint: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_validation() {
        assert!(validate_name(" my-drive ").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("a\\b").is_err());
    }
}
