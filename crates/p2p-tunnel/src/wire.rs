//! /p2p-base/tunnel/1 wire 契约（冻结契约 §1-§3）：票据帧、应答帧、错误码闭集。
//!
//! 帧序：首帧 = 票据帧（JSON UTF-8，≤4 KiB）→ 一帧应答（ack/error）→ 双向字节
//! 分块（首帧之后全部走底座帧封装，单帧上限 1 MiB）。身份不进票据：对端身份
//! 取底座安全握手 PeerId，票据内自报身份一律不采信（契约 §2）。

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 协议 ID 唯一定义点（契约 §1）。
pub const PROTOCOL_ID: &str = "/p2p-base/tunnel/1";
/// 票据帧载荷上限（契约 §1）。
pub const MAX_TICKET_BYTES: usize = 4 * 1024;
/// 票据 ts 允许窗口（±300s，契约 §2）。
pub const TS_WINDOW_SECS: u64 = 300;

/// 票据帧（契约 §2）。nonce 仅作会话关联与重放检测，不是鉴权凭据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelTicket {
    pub v: u8,
    pub uid: String,
    pub target: String,
    pub nonce: String,
    pub ts: u64,
}

impl TunnelTicket {
    /// 访侧构造入口：uid 16 hex / nonce 32 hex，v=1、ts=当前 unix 秒。
    pub fn new(
        uid: impl Into<String>,
        target: impl Into<String>,
        nonce: impl Into<String>,
    ) -> Result<Self, String> {
        let ticket = Self {
            v: 1,
            uid: uid.into(),
            target: target.into(),
            nonce: nonce.into(),
            ts: now_unix_secs(),
        };
        if !is_hex(&ticket.uid, 16) {
            return Err(format!("uid must be 16 hex, got {:?}", ticket.uid));
        }
        if !is_hex(&ticket.nonce, 32) {
            return Err(format!("nonce must be 32 hex, got {:?}", ticket.nonce));
        }
        Ok(ticket)
    }

    /// 覆写 ts（测试构造过期/未来票据用；生产构造即当前时刻）。
    pub fn with_ts(mut self, ts: u64) -> Self {
        self.ts = ts;
        self
    }

    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// 结构 + 策略校验（契约 §2）：版本/uid/nonce/ts 越窗 → BadTicket；
    /// target 非回环字面量形态 → TargetNotAllowed。
    pub fn validate(&self, now: u64, window: u64) -> Result<(), TunnelErrorCode> {
        if self.v != 1 || !is_hex(&self.uid, 16) || !is_hex(&self.nonce, 32) {
            return Err(TunnelErrorCode::BadTicket);
        }
        if self.ts.abs_diff(now) > window {
            return Err(TunnelErrorCode::BadTicket);
        }
        if !is_loopback_literal_target(&self.target) {
            return Err(TunnelErrorCode::TargetNotAllowed);
        }
        Ok(())
    }
}

/// 只允许回环字面量 `127.0.0.1:<port>`（port 1-65535）；localhost/IPv6 等写法一律拒绝。
pub fn is_loopback_literal_target(target: &str) -> bool {
    let Some((host, port)) = target.rsplit_once(':') else {
        return false;
    };
    host == "127.0.0.1" && matches!(port.parse::<u16>(), Ok(p) if p != 0)
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// unix 秒（时钟未就绪回 0，仅用于 ts/审计字段）。
pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 错误码闭集（契约 §3）：wire 字面量见 [Self::as_str]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelErrorCode {
    BadTicket,
    TargetNotAllowed,
    Busy,
    DialFailed,
    Io,
    Shutdown,
}

impl TunnelErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BadTicket => "bad_ticket",
            Self::TargetNotAllowed => "target_not_allowed",
            Self::Busy => "busy",
            Self::DialFailed => "dial_failed",
            Self::Io => "io",
            Self::Shutdown => "shutdown",
        }
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "bad_ticket" => Some(Self::BadTicket),
            "target_not_allowed" => Some(Self::TargetNotAllowed),
            "busy" => Some(Self::Busy),
            "dial_failed" => Some(Self::DialFailed),
            "io" => Some(Self::Io),
            "shutdown" => Some(Self::Shutdown),
            _ => None,
        }
    }
}

impl fmt::Display for TunnelErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for TunnelErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TunnelErrorCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_wire(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown tunnel error code: {s}")))
    }
}

/// 应答帧（契约 §3）：恰一帧，ack 或 error；拒绝路径必须显式回 error 帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum TunnelReply {
    Ack { uid: String },
    Error {
        code: TunnelErrorCode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        msg: Option<String>,
    },
}

impl TunnelReply {
    pub fn ack(uid: &str) -> Self {
        Self::Ack {
            uid: uid.to_string(),
        }
    }

    pub fn error(code: TunnelErrorCode, message: impl Into<String>) -> Self {
        let message = message.into();
        let msg = (!message.is_empty()).then_some(message);
        Self::Error { code, msg }
    }

    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket() -> TunnelTicket {
        TunnelTicket::new("0011223344556677", "127.0.0.1:8014", "ab".repeat(16)).unwrap()
    }

    #[test]
    fn ticket_json_roundtrip_matches_contract_shape() {
        let ticket = ticket();
        let json: serde_json::Value =
            serde_json::from_slice(&ticket.encode().unwrap()).unwrap();
        assert_eq!(json["v"], 1);
        assert_eq!(json["uid"], "0011223344556677");
        assert_eq!(json["target"], "127.0.0.1:8014");
        assert_eq!(json["nonce"], "ab".repeat(16));
        assert!(json["ts"].is_u64());
        assert_eq!(
            TunnelTicket::decode(&ticket.encode().unwrap()).unwrap(),
            ticket
        );
        assert!(TunnelTicket::new("short", "127.0.0.1:80", "ab".repeat(16)).is_err());
        assert!(TunnelTicket::new("0011223344556677", "127.0.0.1:80", "xyz").is_err());
    }

    #[test]
    fn validate_maps_version_ts_and_host_policy() {
        let (now, ok) = (1_000_000u64, ticket().with_ts(1_000_000));
        assert_eq!(ok.validate(now, TS_WINDOW_SECS), Ok(()));
        assert_eq!(ok.validate(now + 300, TS_WINDOW_SECS), Ok(()));
        let mut bad_version = ok.clone();
        bad_version.v = 2;
        assert_eq!(
            bad_version.validate(now, TS_WINDOW_SECS),
            Err(TunnelErrorCode::BadTicket)
        );
        for ts in [now - 301, now + 301] {
            assert_eq!(
                ok.clone().with_ts(ts).validate(now, TS_WINDOW_SECS),
                Err(TunnelErrorCode::BadTicket)
            );
        }
        for host in [
            "localhost:80",
            "[::1]:80",
            "127.0.0.2:80",
            "127.0.0.1",
            "127.0.0.1:0",
            "example.com:80",
        ] {
            let mut bad = ok.clone();
            bad.target = host.into();
            assert_eq!(
                bad.validate(now, TS_WINDOW_SECS),
                Err(TunnelErrorCode::TargetNotAllowed),
                "{host}"
            );
        }
    }

    #[test]
    fn error_code_wire_literals_are_the_contract_closed_set() {
        let all = [
            (TunnelErrorCode::BadTicket, "bad_ticket"),
            (TunnelErrorCode::TargetNotAllowed, "target_not_allowed"),
            (TunnelErrorCode::Busy, "busy"),
            (TunnelErrorCode::DialFailed, "dial_failed"),
            (TunnelErrorCode::Io, "io"),
            (TunnelErrorCode::Shutdown, "shutdown"),
        ];
        for (code, literal) in all {
            assert_eq!(code.as_str(), literal);
            assert_eq!(TunnelErrorCode::from_wire(literal), Some(code));
            assert_eq!(
                serde_json::to_value(code).unwrap(),
                serde_json::Value::from(literal)
            );
        }
        assert_eq!(TunnelErrorCode::from_wire("nope"), None);
    }

    #[test]
    fn reply_frames_match_contract_shapes() {
        let ack = TunnelReply::ack("0011223344556677");
        assert_eq!(
            ack.encode().unwrap(),
            br#"{"k":"ack","uid":"0011223344556677"}"#.to_vec()
        );
        let err = TunnelReply::error(TunnelErrorCode::BadTicket, "wrong v");
        assert_eq!(
            err.encode().unwrap(),
            br#"{"k":"error","code":"bad_ticket","msg":"wrong v"}"#.to_vec()
        );
        assert_eq!(
            TunnelReply::error(TunnelErrorCode::Busy, "").encode().unwrap(),
            br#"{"k":"error","code":"busy"}"#.to_vec()
        );
        assert_eq!(TunnelReply::decode(&ack.encode().unwrap()).unwrap(), ack);
        assert_eq!(TunnelReply::decode(&err.encode().unwrap()).unwrap(), err);
    }
}
