//! /rd/control/1 控制消息模型（remote-desktop-plan §3.1）。
//! JSON 信封 `{"type":"<msg>", ...}`；decode 对非法/越界输入一律拒绝。

use serde::{Deserialize, Serialize};

use crate::{WireError, PROTOCOL_VERSION};

/// 剪贴板文本上限（防对端灌爆内存）。
pub const MAX_CLIPBOARD_BYTES: usize = 8 << 20;
/// 会话 id：16 hex 字符（沿 p2p-tunnel uid 惯例，访问侧生成）。
pub const SESSION_ID_LEN: usize = 16;

/// 会话角色：hello 阶段协商，主动拨号方默认 viewer。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Host,
    Viewer,
}

/// 能力位（hello 双向声明，接收方按交集启用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caps {
    pub audio: bool,
    pub file: bool,
    pub clipboard: bool,
}

/// 显示器拓扑条目（display_list / display_select 用）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: u16,
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
    pub primary: bool,
}

/// 控制消息全集（v1）。type tag 即 `type` 字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMsg {
    /// 握手：viewer 先发；v 未知即 hello_ack 拒绝。
    Hello {
        v: u8,
        role: Role,
        session_id: String,
        caps: Caps,
    },
    /// 握手应答：ok=false 带结构化 reason，随后会话关闭。
    HelloAck {
        ok: bool,
        reason: Option<String>,
        session_id: String,
    },
    /// 鼠标事件：绝对坐标 + 按键掩码（bit0 左/bit1 右/bit2 中）+ 滚轮增量。
    InputMouse {
        x: u16,
        y: u16,
        buttons: u8,
        wheel_dx: i8,
        wheel_dy: i8,
    },
    /// 键盘事件：USB HID 键码，down=true 按下；modifiers 位掩码（0x1 shift/0x2 ctrl/0x4 alt/0x8 meta）。
    InputKey {
        code: u16,
        down: bool,
        modifiers: u8,
    },
    /// 释放全部按键（viewer 失焦/断线时 host MUST 重置）。
    InputKeyReset,
    /// 剪贴板文本同步（≤ MAX_CLIPBOARD_BYTES UTF-8 字节）。
    Clipboard { text: String },
    /// 显示器拓扑（host→viewer，连接建立与拓扑变化时推送）。
    DisplayList {
        displays: Vec<DisplayInfo>,
        current: u16,
    },
    /// 选择捕获目标显示器。
    DisplaySelect { id: u16 },
    /// 质量协商：fps/缩放百分比/编码 codec（video.rs 的 codec 常量）。
    Quality { fps: u8, scale: u8, codec: u8 },
    /// 采纳档位（host→viewer）。
    QualityAck { fps: u8, scale: u8, codec: u8 },
    /// 保活（5s 无心跳判活，宿主按策略关闭）。
    Heartbeat { seq: u64 },
    /// 显式会话关闭。
    Close { reason: String },
}

impl ControlMsg {
    /// 编码并先做语义校验：非法消息不落线。
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        validate(self)?;
        Ok(serde_json::to_vec(self)?)
    }

    /// 解码 + 语义校验：任何非法输入 MUST 拒绝。
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let msg: ControlMsg = serde_json::from_slice(bytes)?;
        validate(&msg)?;
        Ok(msg)
    }
}

/// 语义校验：版本、会话 id 形态、剪贴板大小上限。
pub fn validate(msg: &ControlMsg) -> Result<(), WireError> {
    match msg {
        ControlMsg::Hello { v, session_id, .. } => {
            if *v != PROTOCOL_VERSION {
                return Err(WireError::Invalid(format!("unsupported hello v={v}")));
            }
            check_session_id(session_id)?;
        }
        ControlMsg::HelloAck { session_id, .. } => check_session_id(session_id)?,
        ControlMsg::Clipboard { text } if text.len() > MAX_CLIPBOARD_BYTES => {
            return Err(WireError::Invalid("clipboard exceeds 8 MiB".into()));
        }
        _ => {}
    }
    Ok(())
}

fn check_session_id(id: &str) -> Result<(), WireError> {
    let ok = id.len() == SESSION_ID_LEN && id.bytes().all(|b| b.is_ascii_hexdigit());
    if !ok {
        return Err(WireError::Invalid(format!("bad session_id: {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_ack_without_reason_roundtrip() {
        let msg = ControlMsg::HelloAck {
            ok: true,
            reason: None,
            session_id: "0123456789abcdef".into(),
        };
        let bytes = msg.encode().unwrap();
        assert_eq!(ControlMsg::decode(&bytes).unwrap(), msg);
    }

    #[test]
    fn oversized_clipboard_rejected() {
        let text = "x".repeat(MAX_CLIPBOARD_BYTES + 1);
        let msg = ControlMsg::Clipboard { text };
        assert!(msg.encode().is_err());
    }
}
