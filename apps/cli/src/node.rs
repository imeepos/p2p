//! node 命令域命令面：start/stop/status/serve。机制在 lifecycle.rs，
//! 本模块只做子命令分派与文本/JSON 输出（文本带 pid=/peer=/addr=/log= 键值行，
//! 供脚本无 JSON 依赖采集；--json 为同源结构的 camelCase 序列化）。

use clap::{Args, Subcommand};

use crate::error::CliResult;
use crate::lifecycle::{self, Report};
use crate::output;

pub const DEFAULT_DATA_DIR: &str = "./p2p-data";

#[derive(Subcommand)]
pub enum NodeCommand {
    /// 查询本机节点运行状态
    Status(DirArgs),
    /// 启动节点守护进程（读取 gui-config.json，缺省用默认配置）
    Start(DirArgs),
    /// 停止节点守护进程（幂等；重复 stop 报未运行，退出码 0）
    Stop(DirArgs),
    /// 守护进程入口（由 start 拉起，不对外文档化）
    #[command(hide = true)]
    Serve(DirArgs),
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
        NodeCommand::Status(a) => emit(a.json, lifecycle::status_report(&a.data_dir).await),
        NodeCommand::Start(a) => emit(a.json, lifecycle::start_report(&a.data_dir).await?),
        NodeCommand::Stop(a) => emit(a.json, lifecycle::stop_report(&a.data_dir).await?),
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

fn emit(json: bool, report: Report) -> CliResult<()> {
    let text = render_text(&report);
    output::emit(json, &report, &text)
}

fn render_text(r: &Report) -> String {
    if r.degraded {
        return format!(
            "节点疑似运行中（pid={} 存活，但控制通道不可达）：{}",
            r.pid.unwrap_or(0),
            r.reason
        );
    }
    if !r.running {
        return match r.stopped {
            Some(true) => format!("已停止节点（pid={}）", r.pid.unwrap_or(0)),
            Some(false) => format!("节点未运行（{}），无需停止", empty_or(r)),
            None => format!("节点未运行（{}）", empty_or(r)),
        };
    }
    running_lines(r).join("\n")
}

fn running_lines(r: &Report) -> Vec<String> {
    let mut lines = vec![match r.already_running {
        Some(true) => format!("节点已在运行（pid={}）", r.pid.unwrap_or(0)),
        Some(false) => format!("节点已启动 pid={}", r.pid.unwrap_or(0)),
        None => format!("节点运行中 pid={}", r.pid.unwrap_or(0)),
    }];
    lines.push(format!("pid={}", r.pid.unwrap_or(0)));
    if let Some(peer) = &r.peer_id {
        lines.push(format!("peer={peer}"));
    }
    lines.extend(r.listen_addrs.iter().map(|a| format!("addr={a}")));
    lines.push(format!("log={}", r.log_path));
    if let Some(uptime) = r.uptime_secs {
        lines.push(format!("uptime={uptime}s"));
    }
    lines
}

fn empty_or(r: &Report) -> &str {
    if r.reason.is_empty() {
        "无 pid 文件"
    } else {
        &r.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_report() -> Report {
        Report {
            running: true,
            already_running: None,
            stopped: None,
            pid: Some(42),
            peer_id: Some("abc".into()),
            listen_addrs: vec!["127.0.0.1/u1".into()],
            uptime_secs: Some(9),
            log_path: "/tmp/l".into(),
            data_dir: "/tmp/d".into(),
            degraded: false,
            reason: String::new(),
        }
    }

    #[test]
    fn status_text_has_greppable_key_value_lines() {
        let text = render_text(&running_report());
        for key in [
            "pid=42",
            "peer=abc",
            "addr=127.0.0.1/u1",
            "log=/tmp/l",
            "uptime=9s",
        ] {
            assert!(text.contains(key), "缺 {key}: {text}");
        }
    }

    #[test]
    fn start_and_status_wording_differ() {
        let mut started = running_report();
        started.already_running = Some(false);
        assert!(render_text(&started).starts_with("节点已启动"));
        started.already_running = Some(true);
        assert!(render_text(&started).starts_with("节点已在运行"));
        assert!(render_text(&running_report()).starts_with("节点运行中"));
    }

    #[test]
    fn stop_text_is_idempotent_friendly() {
        let mut stopped = running_report();
        stopped.running = false;
        stopped.stopped = Some(true);
        assert!(render_text(&stopped).contains("已停止节点"));
        stopped.stopped = Some(false);
        stopped.reason = "无 pid 文件".into();
        assert!(render_text(&stopped).contains("节点未运行"));
    }
}
