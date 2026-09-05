//! group 消息子域：send / history / media file（契约 §7）。
//!
//! send 语义对齐 GUI group_send：校验在 p2p-chat crate 内完成；未全部送达按
//! 退出码 1 失败（R4：禁止静默假成功）。身份互斥同 chat send（D6 裁决）。

use std::time::Duration;

use clap::{Args, Subcommand};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;

use crate::chat::context;
use crate::chat::{emit, runtime_err};

#[derive(Args)]
pub struct SendArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 文本内容（与 --file 二选一）
    #[arg(long)]
    text: Option<String>,
    /// 附件文件路径（与 --text 二选一；kind 默认 file）
    #[arg(long)]
    file: Option<std::path::PathBuf>,
    /// 消息类型：text/image/audio/video/file（默认按载荷推断）
    #[arg(long)]
    kind: Option<String>,
    /// 附件 MIME（默认按扩展名推断）
    #[arg(long)]
    mime: Option<String>,
    /// 附件显示名（默认取文件名）
    #[arg(long)]
    name: Option<String>,
    /// 回复引用的消息 id（可选，对齐 GUI group_send 的 replyTo）
    #[arg(long)]
    reply_to: Option<String>,
    /// 发送整体超时秒数（超时按未送达失败）
    #[arg(long, default_value_t = 30)]
    timeout_secs: u64,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct HistoryArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
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

#[derive(Subcommand)]
pub enum MediaCommand {
    /// 查询群附件落盘绝对路径（对齐 GUI group_media_file 语义）
    File(MediaFileArgs),
}

#[derive(Args)]
pub struct MediaFileArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 消息 id
    #[arg(long)]
    message: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// CLI 侧 kind 解析（ChatKind 为外部类型，按孤儿规则手工映射）。
fn parse_kind(s: &str) -> Result<p2p_chat::ChatKind, CliError> {
    match s {
        "text" => Ok(p2p_chat::ChatKind::Text),
        "image" => Ok(p2p_chat::ChatKind::Image),
        "audio" => Ok(p2p_chat::ChatKind::Audio),
        "video" => Ok(p2p_chat::ChatKind::Video),
        "file" => Ok(p2p_chat::ChatKind::File),
        other => Err(CliError::Runtime(format!(
            "--kind 非法: {other}（可选 text/image/audio/video/file）"
        ))),
    }
}

/// --name 缺省显示名：取路径 basename（help 承诺「默认取文件名」；全路径经
/// media 落盘 sanitize 后不可读，ISSUE 2026-09-05）。无 basename（如 "/"、".."）
/// 时回落全路径字符串，不 panic。
fn default_display_name(file: &std::path::Path) -> String {
    file.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_string_lossy().into_owned())
}

/// 载荷归一：--text/--file 二选一（互斥显式报错，R4 语义显式化）。
fn payload(
    args: &SendArgs,
) -> Result<
    (
        p2p_chat::ChatKind,
        Option<String>,
        Option<p2p_chat::ChatMediaInput>,
    ),
    CliError,
> {
    match (&args.text, &args.file) {
        (Some(_), Some(_)) | (None, None) => Err(CliError::Runtime(
            "必须且只能提供 --text 或 --file 之一".into(),
        )),
        (Some(text), None) => {
            if let Some(kind) = &args.kind {
                if kind != "text" {
                    return Err(CliError::Runtime(format!("--kind {kind} 需要 --file 附件")));
                }
            }
            Ok((p2p_chat::ChatKind::Text, Some(text.clone()), None))
        }
        (None, Some(file)) => {
            let kind = match &args.kind {
                None => p2p_chat::ChatKind::File,
                Some(k) => parse_kind(k)?,
            };
            let name = args
                .name
                .clone()
                .unwrap_or_else(|| default_display_name(file));
            let mime = args
                .mime
                .clone()
                .unwrap_or_else(|| "application/octet-stream".into());
            let data = std::fs::read(file)
                .map_err(|e| CliError::Runtime(format!("读附件失败 {}: {e}", file.display())))?;
            Ok((
                kind,
                None,
                Some(p2p_chat::ChatMediaInput { name, mime, data }),
            ))
        }
    }
}

pub async fn send(args: SendArgs) -> CliResult<()> {
    // 身份进程互斥（D6 裁决）：同数据目录不支持多程序并行，被占即快速失败。
    let _identity =
        p2p_chat::try_lock_identity(std::path::Path::new(&args.data_dir)).map_err(runtime_err)?;
    let ctx = context::open(&args.data_dir).await?;
    let (kind, text, media) = payload(&args)?;
    let group_id = args.group.clone();
    let send = tokio::time::timeout(
        Duration::from_secs(args.timeout_secs),
        ctx.chat
            .group
            .group_send(&group_id, kind, text, media, args.reply_to.clone()),
    )
    .await
    .map_err(|_| CliError::Runtime("发送超时，按未送达失败".into()))?
    .map_err(runtime_err)?;
    let text_out = format!(
        "acked {}/{} delivered={}",
        send.acked, send.recipients, send.delivered
    );
    emit(args.json, &send, &text_out)?;
    if !send.delivered {
        // R4：未全员送达（含全部离线）按失败退出，报告已先行输出
        return Err(CliError::Runtime("群消息未全员送达".into()));
    }
    Ok(())
}

pub async fn history(args: HistoryArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let msgs = ctx
        .chat
        .group
        .group_history(
            &args.group,
            args.before_id.as_deref(),
            args.limit.unwrap_or(0),
        )
        .map_err(runtime_err)?;
    let text = if msgs.is_empty() {
        format!("无群历史消息（group={}）", args.group)
    } else {
        let lines: Vec<String> = msgs
            .iter()
            .map(|m| {
                format!(
                    "{} {} {:?} {}",
                    m.ts_ms,
                    m.sender_id,
                    m.kind,
                    m.text.as_deref().unwrap_or("<附件>")
                )
            })
            .collect();
        format!(
            "共 {} 条（group={}）\n{}",
            msgs.len(),
            args.group,
            lines.join("\n")
        )
    };
    emit(args.json, &msgs, &text)
}

pub async fn run_media(command: MediaCommand) -> CliResult<()> {
    let MediaCommand::File(args) = command;
    let ctx = context::open(&args.data_dir).await?;
    let meta = ctx
        .chat
        .group
        .group_media_file(&args.group, &args.message)
        .map_err(runtime_err)?;
    let text = format!(
        "{} {} {}",
        meta.path.clone().unwrap_or_default(),
        meta.mime,
        meta.name
    );
    emit(args.json, &meta, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn display_name_takes_basename() {
        // 纯文件名 / 相对路径 / 绝对路径 / 特殊字符（空格+Unicode+井号）
        assert_eq!(default_display_name(Path::new("shot.png")), "shot.png");
        assert_eq!(
            default_display_name(Path::new("tmp/im-group-drill/shot.png")),
            "shot.png"
        );
        assert_eq!(default_display_name(Path::new("/x/y/shot.png")), "shot.png");
        assert_eq!(
            default_display_name(Path::new("tmp/im-group-drill/截 图#1.png")),
            "截 图#1.png"
        );
    }

    #[test]
    fn display_name_falls_back_without_basename() {
        assert_eq!(default_display_name(Path::new("..")), "..");
        assert_eq!(default_display_name(Path::new("/")), "/");
    }

    #[test]
    fn payload_media_defaults_to_basename() {
        let dir = std::env::temp_dir().join(format!("p2pctl_send_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        let file = dir.join("shot.png");
        std::fs::write(&file, b"png").expect("写临时附件");
        let args = SendArgs {
            group: "g".into(),
            text: None,
            file: Some(file),
            kind: Some("image".into()),
            mime: None,
            name: None,
            reply_to: None,
            timeout_secs: 30,
            json: false,
            data_dir: ".".into(),
        };
        let (_, _, media) = payload(&args).expect("payload 应成功");
        let media = media.expect("--file 必产 media");
        assert_eq!(media.name, "shot.png");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
