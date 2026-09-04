//! send 载荷归一：--text/--file 互斥裁定、kind/MIME 推断、附件读入字节。
//! 校验主体（长度/MIME 白名单/文本规则）在 p2p-chat crate，本层只做 CLI 入参整形。

use std::path::Path;

use p2p_chat::{ChatKind, ChatMediaInput};

use crate::error::CliError;

use super::messages::SendArgs;

/// 载荷归一：--text/--file 二选一（互斥显式报错，R4 语义显式化）。
pub(super) fn payload(
    args: &SendArgs,
) -> Result<(ChatKind, Option<String>, Option<ChatMediaInput>), CliError> {
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
            Ok((ChatKind::Text, Some(text.clone()), None))
        }
        (None, Some(file)) => {
            let kind = match &args.kind {
                None => ChatKind::File,
                Some(k) => parse_kind(k)?,
            };
            let name = args.name.clone().unwrap_or_else(|| fallback_name(file));
            let mime = args.mime.clone().unwrap_or_else(|| guess_mime(&name));
            let data = std::fs::read(file)
                .map_err(|e| CliError::Runtime(format!("读附件失败 {}: {e}", file.display())))?;
            Ok((kind, None, Some(ChatMediaInput { name, mime, data })))
        }
    }
}

/// CLI 侧 kind 解析（ChatKind 为外部类型，按孤儿规则在 CLI 手工映射）。
fn parse_kind(s: &str) -> Result<ChatKind, CliError> {
    match s {
        "text" => Ok(ChatKind::Text),
        "image" => Ok(ChatKind::Image),
        "audio" => Ok(ChatKind::Audio),
        "video" => Ok(ChatKind::Video),
        "file" => Ok(ChatKind::File),
        other => Err(CliError::Runtime(format!(
            "--kind 非法: {other}（可选 text/image/audio/video/file）"
        ))),
    }
}

fn fallback_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "attachment".into())
}

/// 按扩展名推断 MIME；未知扩展名回退 application/octet-stream（file 类型合法）。
fn guess_mime(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/m4a",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    };
    mime.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn args(text: Option<String>, file: Option<PathBuf>) -> SendArgs {
        SendArgs {
            peer: "p".into(),
            text,
            file,
            kind: None,
            mime: None,
            name: None,
            reply_to: None,
            timeout_secs: 30,
            json: false,
            data_dir: "./p2p-data".into(),
        }
    }

    #[test]
    fn payload_requires_exactly_one_of_text_or_file() {
        let err = payload(&args(Some("a".into()), Some(PathBuf::from("b")))).unwrap_err();
        assert!(err.to_string().contains("必须且只能提供"));
        let err = payload(&args(None, None)).unwrap_err();
        assert!(err.to_string().contains("必须且只能提供"));
    }

    #[test]
    fn payload_text_maps_to_text_kind() {
        let (kind, text, media) = payload(&args(Some(" hi ".into()), None)).unwrap();
        assert_eq!(kind, ChatKind::Text);
        assert_eq!(text.as_deref(), Some(" hi "));
        assert!(media.is_none());
    }

    #[test]
    fn payload_file_defaults_to_file_kind_and_reads_bytes() {
        let dir = std::env::temp_dir().join(format!("cl3-payload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("pic.png");
        std::fs::write(&file, b"png").unwrap();
        let (kind, text, media) = payload(&args(None, Some(file))).unwrap();
        assert_eq!(kind, ChatKind::File);
        assert!(text.is_none());
        let media = media.unwrap();
        assert_eq!(media.mime, "image/png");
        assert_eq!(media.data, b"png".to_vec());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn kind_parsing_matrix() {
        assert!(matches!(parse_kind("audio"), Ok(ChatKind::Audio)));
        let err = parse_kind("sticker").unwrap_err();
        assert!(err.to_string().contains("--kind 非法"));
    }

    #[test]
    fn mime_guess_falls_back_to_octet_stream() {
        assert_eq!(guess_mime("a.MOV"), "video/quicktime");
        assert_eq!(guess_mime("noext"), "application/octet-stream");
    }
}