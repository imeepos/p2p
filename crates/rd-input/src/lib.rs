//! rd-input：远程桌面输入注入抽象（remote-desktop-plan §2.3，M3）。
//!
//! [InputInjector] 是 host 侧注入接缝：macOS CGEvent 真实注入
//! （[macos::MacInjector]，需辅助功能授权）与测试用 [recording::RecordingInjector]
//! 可互换。键码走 USB HID usage id（与 /rd/control/1 input_key 一致），
//! [keymap::hid_to_mac_vk] 负责映射；修饰键状态机由注入器自身维护。

pub mod keymap;
pub mod recording;

#[cfg(target_os = "macos")]
pub mod macos;

use std::fmt;

/// 鼠标按键（与 input_mouse.buttons 位掩码对应：bit0 左/bit1 右/bit2 中）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// 从 input_mouse 按键掩码取对应位。
    pub fn from_mask(mask: u8) -> [bool; 3] {
        [mask & 0x01 != 0, mask & 0x02 != 0, mask & 0x04 != 0]
    }
}

/// 修饰键位掩码（与 input_key.modifiers 一致）。
pub mod mods {
    pub const SHIFT: u8 = 0x01;
    pub const CTRL: u8 = 0x02;
    pub const ALT: u8 = 0x04;
    pub const META: u8 = 0x08;
}

/// 注入失败：显式可观测（未映射键码/权限缺失/系统调用失败）。
#[derive(Debug, thiserror::Error)]
pub enum InjectError {
    #[error("key code not mapped: {0:#06x}")]
    UnmappedKey(u16),
    #[error("accessibility permission denied")]
    PermissionDenied,
    #[error("cg event: {0}")]
    Cg(String),
    #[error("io: {0}")]
    Io(String),
}

/// 注入接缝：host 控制处理器逐事件调用；实现须同步完成注入。
pub trait InputInjector: Send {
    fn mouse_move(&mut self, x: u16, y: u16) -> Result<(), InjectError>;
    fn mouse_button(&mut self, btn: MouseButton, down: bool) -> Result<(), InjectError>;
    fn mouse_wheel(&mut self, dx: i8, dy: i8) -> Result<(), InjectError>;
    fn key(&mut self, code: u16, down: bool, modifiers: u8) -> Result<(), InjectError>;
    /// 释放全部按键（viewer 失焦/断线时 host MUST 调用）。
    fn reset_keys(&mut self) -> Result<(), InjectError>;
}

/// 注入器工厂：host 每会话实例化（测试注入 recording，真机注入 CGEvent）。
pub trait InjectorFactory: Send + Sync {
    fn new_injector(&self) -> Result<Box<dyn InputInjector>, InjectError>;
}

/// 诊断辅助：记录型注入器事件（E2E 断言用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    MouseMove {
        x: u16,
        y: u16,
    },
    MouseButton {
        btn: MouseButton,
        down: bool,
    },
    MouseWheel {
        dx: i8,
        dy: i8,
    },
    Key {
        code: u16,
        down: bool,
        modifiers: u8,
    },
    Reset,
}

impl fmt::Display for Recorded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// 修饰键是否在位（纯逻辑，单测覆盖）。
pub fn modifier_bit(modifiers: u8, bit: u8) -> bool {
    modifiers & bit != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_mask_decoding() {
        assert_eq!(MouseButton::from_mask(0b101), [true, false, true]);
        assert_eq!(MouseButton::from_mask(0), [false, false, false]);
    }

    #[test]
    fn modifier_bits() {
        assert!(modifier_bit(mods::CTRL, mods::CTRL));
        assert!(!modifier_bit(mods::META, mods::SHIFT));
    }
}
