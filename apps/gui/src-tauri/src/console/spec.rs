//! 监督参数与退避计算（gui-contract.md §15）：参数可注入，测试用毫秒级退避加速观测。

use std::path::{Path, PathBuf};
use std::time::Duration;

/// stop 轮询间隔：子进程被杀后其孤儿可能持有 stdout 写端使 EOF 迟到，
/// 轮询臂保证收尾在百毫秒级收敛，不依赖管道关闭。
pub(crate) const STOP_POLL: Duration = Duration::from_millis(150);

/// 监督参数：产线用 production()（连续 5 次失败上限、500ms 基数退避封顶 8s）。
#[derive(Clone, Debug)]
pub(crate) struct Limits {
    pub max_consecutive_failures: u32,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
}

impl Limits {
    pub(crate) fn production() -> Self {
        Self {
            max_consecutive_failures: 5,
            backoff_base: Duration::from_millis(500),
            backoff_cap: Duration::from_secs(8),
        }
    }
}

/// 子进程 spawn 规格（产线：随机端口 + 显式数据目录，实际端口从 stdout ready 行读取）。
#[derive(Clone, Debug)]
pub(crate) struct SpawnSpec {
    pub bin: PathBuf,
    pub args: Vec<String>,
}

impl SpawnSpec {
    /// 显式 --data-dir：GUI 从 /Applications 启动时子进程 cwd 为只读根，
    /// acp-console 默认数据目录 ./acp-console-data 会创建失败（2026-09-07 实证）。
    pub(crate) fn production(bin: PathBuf, app_data_dir: &Path) -> Self {
        Self {
            bin,
            args: [
                "--data-dir",
                &app_data_dir.join("acp-console-data").to_string_lossy(),
                "--ws-port",
                "0",
                "--status-port",
                "0",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        }
    }
}

/// 第 n 次连续失败的退避时长：base * 2^(n-1)，封顶 cap（n 从 1 计）。
pub(crate) fn backoff_for(limits: &Limits, consecutive: u32) -> Duration {
    let mut delay = limits.backoff_base;
    let steps = consecutive.saturating_sub(1).min(8);
    for _ in 0..steps {
        let next = delay.saturating_mul(2);
        if next >= limits.backoff_cap {
            return limits.backoff_cap;
        }
        delay = next;
    }
    delay.min(limits.backoff_cap)
}

#[cfg(test)]
mod tests {
    use super::SpawnSpec;

    #[test]
    fn production_args_carry_explicit_data_dir() {
        let spec = SpawnSpec::production(
            std::path::PathBuf::from("/opt/app/acp-console"),
            std::path::Path::new("/tmp/app-data"),
        );
        assert_eq!(spec.bin, std::path::PathBuf::from("/opt/app/acp-console"));
        let joined = spec.args.join(" ");
        assert!(
            joined.contains("--data-dir /tmp/app-data/acp-console-data"),
            "{joined}"
        );
        assert!(joined.contains("--ws-port 0"));
        assert!(joined.contains("--status-port 0"));
    }
}
