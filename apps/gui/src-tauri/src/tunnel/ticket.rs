//! 隧道票据与错误码（冻结契约 §2/§3 逐字）。
//! PROTOCOL_ID 本卡按冻结契约字面量内联定义：W-T2 的 `p2p-tunnel` crate 落地
//! 后，此处改为 `use p2p_tunnel::PROTOCOL_ID`（rebase 对齐点，见 mod.rs 注）。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// 协议 ID（冻结契约 §1；与被访侧逐字一致）。
pub const PROTOCOL_ID: &str = "/p2p-base/tunnel/1";
/// 票据帧上限（冻结契约 §1：JSON ≤4 KiB）。
pub const TICKET_FRAME_MAX: usize = 4096;
/// 票据时间窗（冻结契约 §2：被访侧校验 ±300s）。
pub const TICKET_TS_WINDOW: u64 = 300;
/// uid 长度：16 hex。
const UID_LEN: usize = 16;
/// nonce 长度：32 hex。
const NONCE_LEN: usize = 32;

/// 错误码闭集（冻结契约 §3；wire 字面量四处一致）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelErrorCode {
    BadTicket,
    TargetNotAllowed,
    Busy,
    DialFailed,
    Io,
    Shutdown,
}

impl TunnelErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            TunnelErrorCode::BadTicket => "bad_ticket",
            TunnelErrorCode::TargetNotAllowed => "target_not_allowed",
            TunnelErrorCode::Busy => "busy",
            TunnelErrorCode::DialFailed => "dial_failed",
            TunnelErrorCode::Io => "io",
            TunnelErrorCode::Shutdown => "shutdown",
        }
    }

    pub fn from_wire(raw: &str) -> Option<Self> {
        Some(match raw {
            "bad_ticket" => TunnelErrorCode::BadTicket,
            "target_not_allowed" => TunnelErrorCode::TargetNotAllowed,
            "busy" => TunnelErrorCode::Busy,
            "dial_failed" => TunnelErrorCode::DialFailed,
            "io" => TunnelErrorCode::Io,
            "shutdown" => TunnelErrorCode::Shutdown,
            _ => return None,
        })
    }
}

/// 票据（冻结契约 §2）。字段名即 wire 字面量（serde 默认命名）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelTicket {
    pub v: u8,
    pub uid: String,
    pub target: String,
    pub nonce: String,
    pub ts: u64,
}

impl TunnelTicket {
    /// 访侧构造：uid/nonce 取 OS 随机源（getrandom，控制通道同源）。
    pub fn new_for_target(port: u16) -> Result<Self, String> {
        let mut rand = [0u8; UID_LEN / 2 + NONCE_LEN / 2];
        getrandom::getrandom(&mut rand).map_err(|e| format!("ticket 随机源失败: {e}"))?;
        let uid = hex_encode(&rand[..UID_LEN / 2]);
        let nonce = hex_encode(&rand[UID_LEN / 2..]);
        Ok(Self {
            v: 1,
            uid,
            target: format!("127.0.0.1:{port}"),
            nonce,
            ts: unix_secs()?,
        })
    }

    /// 编码为票据帧载荷（JSON UTF-8）；超 4 KiB 显式报错不静默截断。
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(self).map_err(|e| format!("ticket encode: {e}"))?;
        if bytes.len() > TICKET_FRAME_MAX {
            return Err(format!(
                "ticket frame {} bytes exceeds {TICKET_FRAME_MAX}",
                bytes.len()
            ));
        }
        Ok(bytes)
    }

    /// 解码 + 结构校验（字段取值域照契约 §2；ts 窗口校验由被访侧做）。
    pub fn decode(bytes: &[u8]) -> Result<Self, TunnelErrorCode> {
        if bytes.len() > TICKET_FRAME_MAX {
            return Err(TunnelErrorCode::BadTicket);
        }
        let ticket: Self =
            serde_json::from_slice(bytes).map_err(|_| TunnelErrorCode::BadTicket)?;
        if ticket.v != 1
            || !is_hex_len(&ticket.uid, UID_LEN)
            || !is_hex_len(&ticket.nonce, NONCE_LEN)
            || !is_loopback_target(&ticket.target)
        {
            return Err(TunnelErrorCode::BadTicket);
        }
        Ok(ticket)
    }

    /// 被访侧视角的 ts 窗口校验（±300s，冻结契约 §2）。
    pub fn ts_in_window(&self, now_secs: u64) -> bool {
        now_secs.abs_diff(self.ts) <= TICKET_TS_WINDOW
    }
}

fn is_loopback_target(target: &str) -> bool {
    let Some((host, port_raw)) = target.rsplit_once(':') else {
        return false;
    };
    host == "127.0.0.1"
        && port_raw.parse::<u16>().map(|p| p > 0).unwrap_or(false)
}

fn is_hex_len(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unix_secs() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock before epoch: {e}"))?
        .as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_round_trip_wire_literals() {
        for code in [
            TunnelErrorCode::BadTicket,
            TunnelErrorCode::TargetNotAllowed,
            TunnelErrorCode::Busy,
            TunnelErrorCode::DialFailed,
            TunnelErrorCode::Io,
            TunnelErrorCode::Shutdown,
        ] {
            assert_eq!(TunnelErrorCode::from_wire(code.as_str()), Some(code));
        }
        assert_eq!(TunnelErrorCode::from_wire("other"), None);
    }

    #[test]
    fn ticket_encode_decode_round_trip() {
        let ticket = TunnelTicket::new_for_target(3080).expect("ticket");
        let bytes = ticket.encode().expect("encode");
        assert!(bytes.len() <= TICKET_FRAME_MAX);
        assert_eq!(TunnelTicket::decode(&bytes).expect("decode"), ticket);
    }

    #[test]
    fn decode_rejects_bad_version_target_and_hex() {
        let mut ticket = TunnelTicket::new_for_target(3080).expect("ticket");
        ticket.v = 2;
        assert_eq!(TunnelTicket::decode(&ticket.encode().unwrap()), Err(TunnelErrorCode::BadTicket));
        ticket = TunnelTicket::new_for_target(3080).expect("ticket");
        ticket.target = "10.0.0.1:80".into();
        assert_eq!(TunnelTicket::decode(&ticket.encode().unwrap()), Err(TunnelErrorCode::BadTicket));
        ticket = TunnelTicket::new_for_target(3080).expect("ticket");
        ticket.uid = "xyz".into();
        assert_eq!(TunnelTicket::decode(&ticket.encode().unwrap()), Err(TunnelErrorCode::BadTicket));
        assert_eq!(
            TunnelTicket::decode(b"not json"),
            Err(TunnelErrorCode::BadTicket)
        );
    }

    #[test]
    fn ts_window_is_symmetric_300s() {
        assert!(TunnelTicket { v: 1, uid: "0".repeat(16), target: "127.0.0.1:1".into(), nonce: "0".repeat(32), ts: 1000 }
            .ts_in_window(1299));
        assert!(!TunnelTicket { v: 1, uid: "0".repeat(16), target: "127.0.0.1:1".into(), nonce: "0".repeat(32), ts: 1000 }
            .ts_in_window(1301));
    }
}
