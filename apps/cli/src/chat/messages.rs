//! chat 消息子域：history / send / media file（契约 §12.1）。
//!
//! send 语义（PR1/F3 修订）：delivered → 0；pending（对端离线已排队，恢复后
//! 自动补投）→ 0 且 delivered=false；failed（真失败）→ 先输出报告再退出码 1
//! （R4：禁止静默假成功）。

use std::time::Duration;

use clap::{Args, Subcommand};
use p2p_chat::{ChatEnvelope, ChatStatus, Sender};
use serde::Serialize;

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;

use super::payload::payload;
use super::{context, emit, runtime_err};

#[derive(Args)]
pub struct HistoryArgs {
    /// 对端 peer id
    #[arg(long)]
    peer: String,
    /// 分页游标：只返回严格早于该消息 id 的记录
    #[arg(long)]
    before_id: Option<String>,
    /// 条数（缺省 50，上限 100 由 crate 收敛）
    #[arg(long)]
    limit: Option<usize>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct SendArgs {
    /// 对端 peer id
    #[arg(long)]
    pub(crate) peer: String,
    /// 文本内容（与 --file 二选一）
    #[arg(long)]
    pub(crate) text: Option<String>,
    /// 附件文件路径（与 --text 二选一；kind 默认 file）
    #[arg(long)]
    pub(crate) file: Option<std::path::PathBuf>,
    /// 消息类型：text/image/audio/video/file（默认按载荷推断）
    #[arg(long)]
    pub(crate) kind: Option<String>,
    /// 附件 MIME（默认按扩展名推断）
    #[arg(long)]
    pub(crate) mime: Option<String>,
    /// 附件显示名（默认取文件名）
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// 回复引用的消息 id（可选，对齐 GUI chat_send 的 replyTo）
    #[arg(long)]
    pub(crate) reply_to: Option<String>,
    /// 送达后补投积压预算秒数（上限 30；对端离线按 pending 排队立即返回）
    #[arg(long, default_value_t = 30)]
    pub(crate) timeout_secs: u64,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    pub(crate) json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub(crate) data_dir: String,
}

#[derive(Subcommand)]
pub enum MediaCommand {
    /// 查询附件落盘绝对路径（对齐 GUI chat_media_file 语义）
    File(MediaFileArgs),
}

#[derive(Args)]
pub struct MediaFileArgs {
    /// 对端 peer id
    #[arg(long)]
    peer: String,
    /// 消息 id（history 输出中的 id 字段）
    #[arg(long)]
    message_id: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// send 返回包装：核心报告 + 本轮补投的积压条目数（D3/D5 可观测，契约加法字段）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendReport {
    #[serde(flatten)]
    report: p2p_chat::ChatSendReport,
    flushed_outbox: usize,
}

/// media file 返回（path 为本机落盘绝对路径；GUI 转 asset URL 属前端语义，不进 CLI）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaFileReport {
    path: String,
    mime: String,
    name: String,
}

pub async fn history(args: HistoryArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let msgs = ctx
        .chat
        .history(
            &args.peer,
            args.before_id.as_deref(),
            args.limit.unwrap_or(0),
        )
        .map_err(runtime_err)?;
    let text = if msgs.is_empty() {
        format!("无历史消息（peer={}）", args.peer)
    } else {
        let lines: Vec<String> = msgs.iter().map(fmt_envelope).collect();
        format!(
            "共 {} 条（peer={}）\n{}",
            msgs.len(),
            args.peer,
            lines.join("\n")
        )
    };
    emit(args.json, &msgs, &text)
}

pub async fn send(args: SendArgs) -> CliResult<()> {
    // 身份进程互斥（D6 裁决）：同数据目录不支持多程序并行，被占即快速失败。
    let _identity =
        p2p_chat::try_lock_identity(std::path::Path::new(&args.data_dir)).map_err(runtime_err)?;
    let (kind, text, media) = payload(&args)?;
    let ctx = context::open(&args.data_dir).await?;
    let report = ctx
        .chat
        .send(&args.peer, kind, text, media, args.reply_to.clone())
        .await
        .map_err(runtime_err)?;
    // 送达后顺手排空历史积压（D5）：预算取 --timeout-secs（上限 30s）；离线对端
    // 连接失败立即返回，自动补投由 outbox 补投泵/对端连入窗口收敛，不再原地轮询。
    let mut flushed_outbox = 0usize;
    if report.delivered {
        let budget = Duration::from_secs(args.timeout_secs.min(30));
        flushed_outbox = ctx.chat.drain_peer(&args.peer, budget).await.unwrap_or(0);
    }
    let report = SendReport {
        flushed_outbox,
        report,
    };
    let tail = if flushed_outbox > 0 {
        format!("，另补投积压 {flushed_outbox} 条")
    } else {
        String::new()
    };
    // 退出码语义（PR1/F3）：delivered → 0；pending → 0（已排队，恢复可达自动补投，
    // delivered=false 供自动化判定）；failed → 1（真失败，本机记录保留可手动补投）。
    let text_out = match report.report.message.status {
        ChatStatus::Delivered => format!(
            "已送达 peer={} id={}{}",
            args.peer, report.report.message.id, tail
        ),
        ChatStatus::Failed => format!(
            "未送达 peer={} id={} status=Failed（可经 chat outbox flush 手动补投）",
            args.peer, report.report.message.id
        ),
        other => format!(
            "已排队 peer={} id={} status={other:?}（对端离线，恢复可达后自动补投）",
            args.peer, report.report.message.id
        ),
    };
    emit(args.json, &report, &text_out)?;
    if report.report.message.status == ChatStatus::Failed {
        return Err(CliError::Runtime(
            "消息投递失败（status=Failed），已保留本机记录，可经 chat outbox flush 手动补投"
                .into(),
        ));
    }
    Ok(())
}

pub async fn run_media(command: MediaCommand) -> CliResult<()> {
    match command {
        MediaCommand::File(args) => media_file(args).await,
    }
}

async fn media_file(args: MediaFileArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let meta = ctx
        .chat
        .media_file(&args.peer, &args.message_id)
        .map_err(runtime_err)?;
    let path = meta
        .path
        .clone()
        .ok_or_else(|| CliError::Runtime("附件落盘路径缺失".into()))?;
    let report = MediaFileReport {
        path,
        mime: meta.mime,
        name: meta.name,
    };
    let text = format!("附件路径 {}", report.path);
    emit(args.json, &report, &text)
}

fn fmt_envelope(m: &ChatEnvelope) -> String {
    let who = match m.sender {
        Sender::Me => "我",
        Sender::Them => "对方",
    };
    let body = m.text.clone().unwrap_or_else(|| match &m.media {
        Some(meta) => format!("媒体 [{}] {}", meta.mime, meta.name),
        None => "(空消息)".into(),
    });
    format!("[{}] {} {} {:?}: {}", m.ts_ms, who, m.kind, m.status, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_file_report_json_shape() {
        let report = MediaFileReport {
            path: "/tmp/x.png".into(),
            mime: "image/png".into(),
            name: "x.png".into(),
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["path"], serde_json::json!("/tmp/x.png"));
        assert_eq!(v["mime"], serde_json::json!("image/png"));
    }


    #[test]
    fn envelope_text_line_marks_sender_side() {
        let env = ChatEnvelope {
            id: "m1".into(),
            peer: "p".into(),
            sender: Sender::Them,
            kind: p2p_chat::ChatKind::Text,
            ts_ms: 7,
            text: Some("hi".into()),
            media: None,
            status: p2p_chat::ChatStatus::Delivered,
            reply_to: None,
        };
        let line = fmt_envelope(&env);
        assert!(line.contains("对方"));
        assert!(line.contains("hi"));
    }
}
