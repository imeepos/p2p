//! node 命令域命令面：start/stop/status/serve。机制在 lifecycle.rs，
//! 报告结构与文本/JSON 渲染在 report.rs；本模块只做子命令分派。

use clap::{Args, Subcommand};

use crate::error::{CliError, CliResult};
use crate::lifecycle;
use crate::output;
use crate::report;

pub const DEFAULT_DATA_DIR: &str = "./p2p-data";

#[derive(Subcommand)]
pub enum NodeCommand {
    /// 查询本机节点运行状态
    Status(DirArgs),
    /// 启动节点守护进程（读取 gui-config.json，缺省用默认配置）
    Start(DirArgs),
    /// 停止节点守护进程（幂等；重复 stop 报未运行，退出码 0）
    Stop(DirArgs),
    /// 守护进程日志只读查询（F7）：读 <data-dir>/daemon.log
    Log {
        #[command(subcommand)]
        command: NodeLogCommand,
    },
    /// 守护进程入口（由 start 拉起，不对外文档化）
    #[command(hide = true)]
    Serve(DirArgs),
}

/// node log 子命令：tail 语义对齐 log 域（F7 命名对齐 SRE 直觉）。
#[derive(Subcommand)]
pub enum NodeLogCommand {
    /// 读守护进程日志 daemon.log 末尾 N 行（默认 200，上限 1000，同 log tail）
    Tail(LogTailArgs),
}

#[derive(Args)]
pub struct LogTailArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录（日志位于 <data-dir>/daemon.log）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
    /// 读取行数（默认 200，上限 1000）
    #[arg(long)]
    lines: Option<u32>,
}

/// log tail 报告（与 log 域 TailReport 同形）。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LogTailReport {
    path: String,
    lines: Vec<String>,
}

#[derive(Args)]
pub struct DirArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录（等价 GUI app 数据目录，含 gui-config.json/daemon.*）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

impl DirArgs {
    pub fn new(json: bool, data_dir: String) -> Self {
        Self { json, data_dir }
    }
}

pub async fn run(cmd: NodeCommand) -> CliResult<()> {
    match cmd {
        NodeCommand::Status(a) => report::emit(a.json, lifecycle::status_report(&a.data_dir).await),
        NodeCommand::Start(a) => report::emit(a.json, lifecycle::start_report(&a.data_dir).await?),
        NodeCommand::Stop(a) => report::emit(a.json, lifecycle::stop_report(&a.data_dir).await?),
        NodeCommand::Log { command } => match command {
            NodeLogCommand::Tail(a) => log_tail(a),
        },
        NodeCommand::Serve(a) => crate::daemon::run(&a.data_dir).await,
    }
}

/// 供其他命令域复用：数据目录对应节点在线则完整停机，返回是否停了节点。
pub async fn stop_if_online(data_dir: &str) -> CliResult<bool> {
    if lifecycle::probe_online(data_dir).await.is_none() {
        return Ok(false);
    }
    run(NodeCommand::Stop(DirArgs::new(false, data_dir.to_string()))).await?;
    Ok(true)
}

/// node log tail（F7）：daemon.log 只读尾读，语义对齐 log tail
/// （默认 200 行、上限 1000、文件缺失不算错——首启前为空是合法状态）。
fn log_tail(args: LogTailArgs) -> CliResult<()> {
    use crate::paths::Paths;
    let paths = Paths::new(&args.data_dir);
    let path = paths.log();
    let max = args.lines.unwrap_or(crate::log::DEFAULT_TAIL_LINES).max(1) as usize;
    let lines = crate::log::backend::tail_lines(&path, max)
        .map_err(|e| CliError::Runtime(format!("守护进程日志读取失败: {e}")))?;
    let report = LogTailReport {
        path: path.to_string_lossy().into_owned(),
        lines,
    };
    let text = report.lines.join("\n");
    output::emit(args.json, &report, &text)
}
