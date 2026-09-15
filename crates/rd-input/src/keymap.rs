//! USB HID usage id → macOS 虚拟键码（CGKeyCode）映射表。
//! 映射不完整即 None（宿主丢弃并告警，不猜测）；覆盖日常全键位。

/// USB HID 键盘页 usage id（子集：映射表覆盖范围）。
pub mod hid {
    pub const A: u16 = 0x04;
    pub const Z: u16 = 0x1D;
    pub const D1: u16 = 0x1E;
    pub const D0: u16 = 0x27;
    pub const RETURN: u16 = 0x28;
    pub const ESCAPE: u16 = 0x29;
    pub const BACKSPACE: u16 = 0x2A;
    pub const TAB: u16 = 0x2B;
    pub const SPACE: u16 = 0x2C;
    pub const MINUS: u16 = 0x2D;
    pub const EQUAL: u16 = 0x2E;
    pub const LEFT_BRACKET: u16 = 0x2F;
    pub const RIGHT_BRACKET: u16 = 0x30;
    pub const BACKSLASH: u16 = 0x31;
    pub const SEMICOLON: u16 = 0x33;
    pub const GRAVE: u16 = 0x35;
    pub const COMMA: u16 = 0x36;
    pub const PERIOD: u16 = 0x37;
    pub const SLASH: u16 = 0x38;
    pub const CAPS_LOCK: u16 = 0x39;
    pub const F1: u16 = 0x3A;
    pub const F12: u16 = 0x45;
    pub const RIGHT_ARROW: u16 = 0x4F;
    pub const LEFT_ARROW: u16 = 0x50;
    pub const DOWN_ARROW: u16 = 0x51;
    pub const UP_ARROW: u16 = 0x52;
    pub const KEYPAD_ENTER: u16 = 0x58;
    pub const LEFT_CTRL: u16 = 0xE0;
    pub const LEFT_SHIFT: u16 = 0xE1;
    pub const LEFT_ALT: u16 = 0xE2;
    pub const LEFT_META: u16 = 0xE3;
    pub const RIGHT_CTRL: u16 = 0xE4;
    pub const RIGHT_SHIFT: u16 = 0xE5;
    pub const RIGHT_ALT: u16 = 0xE6;
    pub const RIGHT_META: u16 = 0xE7;
}

/// 修饰键 usage id 集合（注入器状态机按此识别）。
pub fn is_modifier(code: u16) -> bool {
    matches!(
        code,
        hid::LEFT_CTRL
            | hid::LEFT_SHIFT
            | hid::LEFT_ALT
            | hid::LEFT_META
            | hid::RIGHT_CTRL
            | hid::RIGHT_SHIFT
            | hid::RIGHT_ALT
            | hid::RIGHT_META
            | hid::CAPS_LOCK
    )
}

/// 修饰键 usage id → 对应 wire 位掩码。
pub fn modifier_mask(code: u16) -> u8 {
    match code {
        hid::LEFT_SHIFT | hid::RIGHT_SHIFT => crate::mods::SHIFT,
        hid::LEFT_CTRL | hid::RIGHT_CTRL => crate::mods::CTRL,
        hid::LEFT_ALT | hid::RIGHT_ALT => crate::mods::ALT,
        hid::LEFT_META | hid::RIGHT_META => crate::mods::META,
        _ => 0,
    }
}

/// USB HID → macOS 虚拟键码；未映射返回 None（调用方显式丢弃+告警）。
pub fn hid_to_mac_vk(code: u16) -> Option<u16> {
    match code {
        // 字母区：HID 0x04..=0x1D → kVK_ANSI_A(0x00).. 连续
        hid::A..=hid::Z => Some(code - hid::A),
        // 数字行：HID 0x1E..=0x27 → kVK_ANSI_1(0x12).. 连续
        hid::D1..=hid::D0 => Some(0x12 + (code - hid::D1)),
        hid::RETURN => Some(0x24),
        hid::ESCAPE => Some(0x35),
        hid::BACKSPACE => Some(0x33),
        hid::TAB => Some(0x30),
        hid::SPACE => Some(0x31),
        hid::MINUS => Some(0x1B),
        hid::EQUAL => Some(0x18),
        hid::LEFT_BRACKET => Some(0x21),
        hid::RIGHT_BRACKET => Some(0x1E),
        hid::BACKSLASH => Some(0x2A),
        hid::SEMICOLON => Some(0x29),
        hid::GRAVE => Some(0x32),
        hid::COMMA => Some(0x2B),
        hid::PERIOD => Some(0x2F),
        hid::SLASH => Some(0x2C),
        hid::CAPS_LOCK => Some(0x39),
        hid::F1..=hid::F12 => Some(F_KEYS[(code - hid::F1) as usize]),
        hid::RIGHT_ARROW => Some(0x7C),
        hid::LEFT_ARROW => Some(0x7B),
        hid::DOWN_ARROW => Some(0x7D),
        hid::UP_ARROW => Some(0x7E),
        hid::KEYPAD_ENTER => Some(0x4C),
        hid::LEFT_CTRL => Some(0x3B),
        hid::LEFT_SHIFT => Some(0x38),
        hid::LEFT_ALT => Some(0x3A),
        hid::LEFT_META => Some(0x37),
        hid::RIGHT_CTRL => Some(0x3E),
        hid::RIGHT_SHIFT => Some(0x3C),
        hid::RIGHT_ALT => Some(0x3D),
        hid::RIGHT_META => Some(0x36),
        _ => None,
    }
}

/// F1..F12 的 macOS 虚拟键码（非连续，查表）。
const F_KEYS: [u16; 12] = [
    0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_map_consecutively() {
        assert_eq!(hid_to_mac_vk(hid::A), Some(0x00));
        assert_eq!(hid_to_mac_vk(hid::Z), Some(0x19));
        assert_eq!(hid_to_mac_vk(hid::D1), Some(0x12));
        assert_eq!(hid_to_mac_vk(hid::D0), Some(0x1B));
    }

    #[test]
    fn function_keys_table() {
        assert_eq!(hid_to_mac_vk(hid::F1), Some(0x7A));
        assert_eq!(hid_to_mac_vk(hid::F12), Some(0x6F));
    }

    #[test]
    fn modifiers_recognized() {
        for c in [
            hid::LEFT_SHIFT,
            hid::RIGHT_CTRL,
            hid::LEFT_META,
            hid::RIGHT_ALT,
        ] {
            assert!(is_modifier(c));
        }
        assert!(!is_modifier(hid::A));
        assert_eq!(modifier_mask(hid::LEFT_SHIFT), crate::mods::SHIFT);
        assert_eq!(modifier_mask(hid::RIGHT_ALT), crate::mods::ALT);
    }

    #[test]
    fn unmapped_is_none() {
        assert_eq!(hid_to_mac_vk(0x46), None); // PrintScreen 未映射
        assert_eq!(hid_to_mac_vk(0x47), None); // ScrollLock 未映射
        assert_eq!(hid_to_mac_vk(0xFFFF), None);
    }
}
