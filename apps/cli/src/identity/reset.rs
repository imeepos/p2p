//! identity reset：停节点 + 删 key.seed（危险操作，语义同 GUI identity_reset）。

use serde::Serialize;

use crate::error::{CliError, CliResult};
use crate::lifecycle;
use crate::node;
use crate::output;
use crate::paths::Paths;
use crate::store;

use super::ResetArgs;

/// 重置结论：文本/JSON 共用。
#[derive(Serialize)]
struct ResetReport {
    reset: bool,
    stopped_node: bool,
    seed_path: String,
    seed_removed: bool,
    online_after: bool,
}

pub(super) async fn reset(args: ResetArgs) -> CliResult<()> {
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
    let text = format!(
        "身份已重置（stoppedNode={stopped_node}）\nseed={}",
        report.seed_path
    );
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
        let dir = std::env::temp_dir().join(format!("p2pctl-id-rm-{}", std::process::id()));
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
