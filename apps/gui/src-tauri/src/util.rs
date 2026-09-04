//! 小工具：毫秒时间戳（事件盖戳与状态快照共用）与媒体 asset URL 转换。

use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 Unix 毫秒时间戳；系统时钟早于纪元时返回 0（仅损失排序，不 panic）。
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// 把本端落盘绝对路径转成 Tauri asset protocol URL（镜像前端 convertFileSrc）：
/// 非 Windows/Android：`asset://localhost/<encodeURIComponent>`；Windows：`http://asset.localhost/<...>`。
/// 前端 MediaContent 只接受 https:/blob:/data:/asset: 前缀（T31 接缝），裸路径不可内联。
pub fn to_asset_url(path: &str) -> String {
    let encoded = encode_uri_component(path);
    if cfg!(target_os = "windows") {
        format!("http://asset.localhost/{encoded}")
    } else {
        format!("asset://localhost/{encoded}")
    }
}

/// 把消息里的媒体落盘路径替换为 asset URL（crate 内部仍存绝对路径，仅输出边界转换）。
pub fn to_asset_media(mut env: p2p_chat::ChatEnvelope) -> p2p_chat::ChatEnvelope {
    if let Some(media) = &mut env.media {
        if let Some(p) = &media.path {
            media.path = Some(to_asset_url(p));
        }
    }
    env
}

/// encodeURIComponent 语义：除 `A-Za-z0-9-_.!~*'()` 外逐 UTF-8 字节 %XX 大写编码。
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_url_uses_asset_scheme_with_encoded_path() {
        let url = to_asset_url("/data/chat/media/PeerA/id-1_photo.png");
        assert!(url.starts_with("asset://localhost/"), "实际: {url}");
        // 前缀之后的编码段不含裸 /：整条绝对路径被 encodeURIComponent 处理
        let encoded = url.trim_start_matches("asset://localhost/");
        assert!(encoded.contains("%2F"), "路径分隔符须编码: {encoded}");
        assert!(!encoded.contains('/'), "编码段不应有裸 /: {encoded}");
        assert!(encoded.contains("id-1_photo.png"), "文件名保留: {encoded}");
    }

    #[test]
    fn encode_uri_component_matches_js_semantics() {
        assert_eq!(encode_uri_component("a b+c"), "a%20b%2Bc");
        assert_eq!(encode_uri_component("_-!.~*'()"), "_-!.~*'()");
        assert_eq!(encode_uri_component("中文"), "%E4%B8%AD%E6%96%87");
    }

    #[test]
    fn to_asset_media_converts_path_only() {
        let env = p2p_chat::ChatEnvelope {
            id: "m1".into(),
            peer: "P".into(),
            sender: p2p_chat::Sender::Me,
            kind: p2p_chat::ChatKind::Image,
            ts_ms: 0,
            text: None,
            media: Some(p2p_chat::ChatMediaMeta {
                name: "a.png".into(),
                mime: "image/png".into(),
                size: 3,
                path: Some("/data/chat/media/P/m1_a.png".into()),
            }),
            status: p2p_chat::ChatStatus::Delivered,
            reply_to: None,
        };
        let out = to_asset_media(env);
        let path = out.media.expect("媒体仍在").path.expect("path 已转换");
        assert!(path.starts_with("asset://localhost/"), "实际: {path}");
        assert!(path.contains("a.png"), "文件名保留: {path}");
    }
}
