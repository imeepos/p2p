//! identity 命令域：对齐 GUI identity_reset。危险操作必须显式 --confirm；
//! 停运行中节点后删除身份数据目录的 key.seed（不存在视为已重置，语义同 GUI）。

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::error::{CliError, CliResult};
use crate::lifecycle;
use crate::node;
use crate::output;
use crate::paths::Paths;
use crate::store;
use crate::node::DEFAULT_DATA_DIR;

#[derive(Subcommand)]
pub enum IdentityCommand {
    /// 重置身份：停节点 + 删除 key.seed；必须显式 --confirm
    Reset(ResetArgs),
}

#[derive(Args)]
pub struct ResetArgs {
    /// 危险操作确认（缺失即拒绝执行）
    #[arg(long)]
    confirm: bool,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// 重置结论：文本/JSON 共用。
#[derive(Serialize)]
struct ResetReport {
    reset: bool,
    stopped_node: bool,
    seed_path: String,
    seed_removed: bool,
    online_after: bool,
}

pub async fn run(cmd: IdentityCommand) -> CliResult<()> {
    match cmd {
        IdentityCommand::Reset(a) => reset(a).await,
    }
}

async fn reset(args: ResetArgs) -> CliResult<()> {
    if !args.confirm {
        return Err(CliError::Runtime(
            "重置身份是危险操作，必须显式传入 --confirm".into(),
        ));
    }
    let paths = Paths::new(&args.data_dir);
    let stopped_node = node::stop_if_online(&args.data_dir).await?;
    let cfg = store::load_config(&paths);
    let seed = paths.node_data_dir(Some(&cfg.data_dir)).join("key.seed");
    let seed_removed = remove_seed(&seed)?;
    let online_after = lifecycle::probe_online(&args.data_dir).await.is_some();
    let report = ResetReport {
        reset: true,
        stopped_node,
        seed_path: seed.to_string_lossy().into_owned(),
        seed_removed,
        online_after,
    };
    let text = format!("身份已重置（stoppedNode={stopped_node}）\nseed={}", report.seed_path);
    output::emit(args.json, &report, &text)
}

fn remove_seed(seed: &std::path::Path) -> CliResult<bool> {
    match std::fs::remove_file(seed) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(CliError::Runtime(format!("删除身份数据失败: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_removal_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("p2pctl-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let seed = dir.join("key.seed");
        assert!(!remove_seed(&seed).unwrap(), "不存在视为已重置");
        std::fs::write(&seed, b"x").unwrap();
        assert!(remove_seed(&seed).unwrap());
        assert!(!remove_seed(&seed).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
