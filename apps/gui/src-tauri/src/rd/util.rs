//! rd 纯函数助手：peer 解析、会话 id、文件根。自 rd.rs 拆出（该文件
//! 行数红线承压，零行为迁移；命令定义仍在 rd.rs 保持 §19 路径字面）。

use std::path::PathBuf;

use p2p_identity::PeerId;

/// 无 Channel 的探测型连接渲染兜底（帧丢弃）。
pub(super) struct NoopSink;
impl rd_viewer::RenderSink for NoopSink {
    fn on_frame(&self, _frame: rd_viewer::DecodedFrame) {}
}

/// peer base58 解析（与 CLI/tunnel 同规则）。
pub(super) fn parse_peer(raw: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(raw.trim())
        .into_vec()
        .map_err(|e| format!("peer 非 base58: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("peer 长度非法（须 32 字节）: {raw}"))?;
    Ok(PeerId::from_bytes(arr))
}

/// 16 位无连字符会话 id（viewer 连接缺省）。
pub(super) fn random_session_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_string()
}

/// host 文件隔离根缺省：`$HOME/Downloads/RD`（HOME 缺失回退 .rd-files）。
pub(super) fn default_fs_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(h) => h.join("Downloads").join("RD"),
        None => PathBuf::from(".rd-files"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_parse_valid_and_invalid() {
        let bytes = [7u8; 32];
        let b58 = bs58::encode(bytes).into_string();
        assert!(parse_peer(&b58).is_ok());
        assert!(parse_peer("not-base58!!").is_err());
        assert!(parse_peer("abc").is_err(), "长度非法");
    }

    #[test]
    fn session_id_is_16_chars() {
        assert_eq!(random_session_id().len(), 16);
    }
}
