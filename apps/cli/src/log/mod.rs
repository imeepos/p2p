//! log 命令域（CL4）：对齐 GUI frontend_log_tail / frontend_log_path / frontend_log_clear。
//! frontend_log_append 是 GUI 前端专属行为（浏览器侧采集源），CLI 不提供，
//! 豁免登记见 scripts/check/cli-parity.tsv。默认读 GUI 日志目录，--log-dir 覆盖。

mod backend;

use clap::Subcommand;
use serde::Serialize;

use crate::error::CliResult;
use crate::output;

use backend::{FrontendLog, DEFAULT_TAIL_LINES};

#[derive(Subcommand)]
pub enum LogCommand {
    /// 读 GUI 前端日志末尾 N 行（默认 200，上限 1000，同 GUI）
    Tail(TailArgs),
    /// 输出 GUI 前端日志文件绝对路径
    Path(LogArgs),
    /// 清理 GUI 前端日志（连轮转代 frontend.log.1 一起删，幂等）
    Clear(LogArgs),
}

#[derive(clap::Args)]
pub struct LogArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// GUI 日志目录覆盖（默认取 GUI 应用日志目录 com.p2p.console）
    #[arg(long)]
    log_dir: Option<String>,
}

#[derive(clap::Args)]
pub struct TailArgs {
    #[command(flatten)]
    common: LogArgs,
    /// 读取行数（默认 200，上限 1000）
    #[arg(long)]
    lines: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathReport {
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearReport {
    cleared: bool,
    removed_current: bool,
    removed_rotated: bool,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TailReport {
    path: String,
    lines: Vec<String>,
}

pub async fn run(cmd: LogCommand) -> CliResult<()> {
    match cmd {
        LogCommand::Tail(a) => tail(a),
        LogCommand::Path(a) => path(a),
        LogCommand::Clear(a) => clear(a),
    }
}

fn tail(args: TailArgs) -> CliResult<()> {
    let log = resolve(&args.common.log_dir)?;
    let max = args.lines.unwrap_or(DEFAULT_TAIL_LINES).max(1) as usize;
    let lines = log
        .tail(max)
        .map_err(|e| crate::error::CliError::Runtime(format!("前端日志读取失败: {e}")))?;
    let report = TailReport { path: path_string(&log), lines };
    let text = report.lines.join("\n");
    output::emit(args.common.json, &report, &text)
}

fn path(args: LogArgs) -> CliResult<()> {
    let log = resolve(&args.log_dir)?;
    let report = PathReport { path: path_string(&log) };
    output::emit(args.json, &report, &report.path)
}

fn clear(args: LogArgs) -> CliResult<()> {
    let log = resolve(&args.log_dir)?;
    let (removed_current, removed_rotated) = log
        .clear()
        .map_err(|e| crate::error::CliError::Runtime(format!("前端日志清理失败: {e}")))?;
    let report = ClearReport {
        cleared: true,
        removed_current,
        removed_rotated,
        path: path_string(&log),
    };
    let text = format!(
        "已清理前端日志 current={} rotated={} path={}",
        removed_current, removed_rotated, report.path
    );
    output::emit(args.json, &report, &text)
}

fn resolve(log_dir: &Option<String>) -> CliResult<FrontendLog> {
    FrontendLog::resolve(log_dir.as_deref())
        .map_err(crate::error::CliError::Runtime)
}

fn path_string(log: &FrontendLog) -> String {
    log.path().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("p2pctl-log-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tail_clear_roundtrip_via_args() {
        let dir = temp_dir("roundtrip");
        let log_dir = dir.to_string_lossy().into_owned();
        let file = dir.join(backend::FRONTEND_LOG_FILE);
        std::fs::write(&file, "a\nb\nc\n").unwrap();

        tail(TailArgs { common: LogArgs { json: false, log_dir: Some(log_dir.clone()) }, lines: Some(2) }).unwrap();
        clear(LogArgs { json: false, log_dir: Some(log_dir.clone()) }).unwrap();
        assert!(!file.exists(), "clear 应删除 frontend.log");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_clamps_lines_to_gui_cap() {
        let dir = temp_dir("cap");
        let log = FrontendLog::resolve(dir.to_str()).unwrap();
        let body: String = (0..1200).map(|i| format!("{i}")).collect::<Vec<_>>().join("\n");
        std::fs::write(log.path(), body).unwrap();
        assert_eq!(log.tail(usize::MAX).unwrap().len(), 1000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
