//! gui 命令域（GC2）：对接 GUI 本地控制通道——状态/截图/录屏/导航/受限 invoke。
//! 通道发现与鉴权见 channel.rs；CLI 单侧能力，不进 cli-parity 映射表（GUI 命令面未变）。

mod channel;

use std::path::PathBuf;

use clap::Args;
use clap::Subcommand;
use serde_json::{Value, json};

use channel::Channel;

use crate::error::{CliError, CliResult};
use crate::output;

#[derive(Subcommand)]
pub enum GuiCommand {
    /// 查询运行中 GUI 的状态（版本/窗口/当前路由）
    Status {
        #[command(flatten)]
        args: OutputArgs,
    },
    /// 截图主窗口内容并落盘 PNG（路径须绝对）
    Screenshot {
        /// PNG 输出绝对路径
        #[arg(short = 'o', long = "output")]
        output: String,
        #[command(flatten)]
        args: OutputArgs,
    },
    /// 录屏控制（采样编码 GIF，产物路径随命令给出）
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// 按路由名切换 GUI 页面（dashboard/peers/discovery/relay/chat/events/settings/diagnostics）
    Navigate {
        /// 路由名（服务端白名单校验）
        route: String,
        #[command(flatten)]
        args: OutputArgs,
    },
    /// 转发白名单内只读命令（node_status/metrics_get/metrics_history/config_get/profile_get）
    Invoke {
        /// Tauri 命令名（服务端白名单校验）
        command: String,
        /// 转发参数，k=v 形态可重复（当前白名单命令均无参数）
        #[arg(long = "arg")]
        args: Vec<String>,
        #[command(flatten)]
        out: OutputArgs,
    },
}

#[derive(Subcommand)]
pub enum RecordCommand {
    /// 开始录屏（产物 GIF，路径须绝对；--interval-ms 200..5000，默认 500）
    Start {
        /// GIF 输出绝对路径
        #[arg(short = 'o', long = "output")]
        output: String,
        /// 采样间隔毫秒
        #[arg(long)]
        interval_ms: Option<u64>,
        #[command(flatten)]
        args: OutputArgs,
    },
    /// 停止录屏并等待产物落盘
    Stop {
        #[command(flatten)]
        args: OutputArgs,
    },
}

/// gui 域通用输出参数：所有子命令支持 --json 与 --gui-data-dir。
#[derive(Args)]
pub struct OutputArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// GUI 数据目录覆盖（默认 macOS: ~/Library/Application Support/com.p2p.console）
    #[arg(long)]
    gui_data_dir: Option<String>,
}

pub async fn run(cmd: GuiCommand) -> CliResult<()> {
    match cmd {
        GuiCommand::Status { args } => status(&args).await,
        GuiCommand::Screenshot { output, args } => screenshot(&args, &output).await,
        GuiCommand::Record { command } => match command {
            RecordCommand::Start { output, interval_ms, args } => {
                record_start(&args, &output, interval_ms).await
            }
            RecordCommand::Stop { args } => record_stop(&args).await,
        },
        GuiCommand::Navigate { route, args } => navigate(&args, &route).await,
        GuiCommand::Invoke { command, args, out } => invoke(&out, &command, &args).await,
    }
}

/// 打开控制通道：显式 --gui-data-dir 优先，否则按 OS 默认应用数据目录。
fn open(args: &OutputArgs) -> CliResult<Channel> {
    let dir = match &args.gui_data_dir {
        Some(d) => PathBuf::from(d),
        None => channel::default_data_dir()?,
    };
    channel::connect(&dir.join("control"))
}

async fn status(args: &OutputArgs) -> CliResult<()> {
    let data = open(args)?.get("/health").await?;
    // health 的 title 即窗口标题，文本键以 window 呈现，其余键与 JSON 同名。
    let pairs = [("version", "version"), ("title", "window"), ("route", "route"),
        ("pid", "pid"), ("uptimeMs", "uptimeMs"), ("recording", "recording")];
    let rows: Vec<String> = pairs.iter().map(|(jk, tk)| format!("{tk}={}", scalar(&data[jk]))).collect();
    output::emit(args.json, &data, &rows.join("\n"))
}

async fn screenshot(args: &OutputArgs, output: &str) -> CliResult<()> {
    let data = open(args)?.post("/screenshot", json!({ "path": output })).await?;
    emit_kv(args, &data, &["path", "width", "height", "bytes"])
}

async fn record_start(args: &OutputArgs, output: &str, interval_ms: Option<u64>) -> CliResult<()> {
    let mut body = json!({ "path": output });
    if let Some(ms) = interval_ms {
        body["intervalMs"] = json!(ms);
    }
    let data = open(args)?.post("/record/start", body).await?;
    emit_kv(args, &data, &["path", "intervalMs"])
}

async fn record_stop(args: &OutputArgs) -> CliResult<()> {
    let data = open(args)?.post("/record/stop", Value::Null).await?;
    emit_kv(args, &data, &["path", "frames", "bytes", "truncated"])
}

async fn navigate(args: &OutputArgs, route: &str) -> CliResult<()> {
    let data = open(args)?.post("/navigate", json!({ "route": route })).await?;
    emit_kv(args, &data, &["route", "path"])
}

async fn invoke(out: &OutputArgs, command: &str, pairs: &[String]) -> CliResult<()> {
    let mut body = json!({ "command": command });
    let args = parse_pairs(pairs)?;
    if !args.is_null() {
        body["args"] = args;
    }
    let data = open(out)?.post("/invoke", body).await?;
    let text = serde_json::to_string_pretty(&data)
        .map_err(|e| CliError::Runtime(format!("invoke 结果序列化失败: {e}")))?;
    output::emit(out.json, &data, &text)
}

/// 统一 key=value 文本渲染（键与 JSON 字段同名，health.title 以 window 键呈现）。
fn emit_kv(args: &OutputArgs, data: &Value, keys: &[&str]) -> CliResult<()> {
    let rows: Vec<String> = keys.iter().map(|k| format!("{k}={}", scalar(&data[*k]))).collect();
    output::emit(args.json, data, &rows.join("\n"))
}

/// 标量渲染：字符串去引号，其余用 JSON 形态，Null 记 ?（异常形态可观测）。
fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "?".to_string(),
        other => other.to_string(),
    }
}

/// k=v 对转 JSON 对象；v 可解析为 JSON 值则保留类型，否则按字符串；空入参返回 Null。
fn parse_pairs(pairs: &[String]) -> CliResult<Value> {
    if pairs.is_empty() {
        return Ok(Value::Null);
    }
    let mut map = serde_json::Map::new();
    for pair in pairs {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| CliError::Runtime(format!("--arg 须为 k=v 形态: {pair}")))?;
        let typed = serde_json::from_str::<Value>(v).unwrap_or(Value::String(v.to_string()));
        map.insert(k.to_string(), typed);
    }
    Ok(Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pairs_types_and_defaults() {
        let pairs = vec!["n=3".to_string(), "flag=true".to_string(), "s=hello".to_string()];
        let v = parse_pairs(&pairs).unwrap();
        assert_eq!(v["n"], json!(3));
        assert_eq!(v["flag"], json!(true));
        assert_eq!(v["s"], json!("hello"));
    }

    #[test]
    fn parse_pairs_empty_is_null_and_bad_shape_errors() {
        assert_eq!(parse_pairs(&[]).unwrap(), Value::Null);
        let err = parse_pairs(&["novalue".to_string()]).unwrap_err();
        assert!(err.to_string().contains("k=v"), "{err}");
    }

    #[test]
    fn kv_render_keeps_json_keys_and_unmasks_null() {
        let data = json!({ "path": "/tmp/x.png", "bytes": 12, "height": Value::Null });
        let text = emit_kv_test(&data, &["path", "bytes", "height", "missing"]);
        assert_eq!(text, "path=/tmp/x.png\nbytes=12\nheight=?\nmissing=?");
    }

    fn emit_kv_test(data: &Value, keys: &[&str]) -> String {
        keys.iter().map(|k| format!("{k}={}", scalar(&data[*k]))).collect::<Vec<_>>().join("\n")
    }
}
