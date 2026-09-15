//! macOS CGEvent 真实注入（需辅助功能授权；权限缺失显式 [InjectError::PermissionDenied]）。
//! 修饰键状态机：host 权威维护已按住修饰键集合，事件 flags 带全量状态。

use std::collections::HashSet;

use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID::HIDSystemState};
use core_graphics::geometry::CGPoint;

use crate::keymap::{hid_to_mac_vk, is_modifier};
use crate::{InjectError, InjectorFactory, InputInjector, MouseButton};

/// 辅助功能授权探测：AXIsProcessTrusted（ApplicationServices 框架直链）。
pub fn accessibility_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// macOS 真实注入器工厂（rd-host 默认装配）。
pub struct MacInjectorFactory;

impl InjectorFactory for MacInjectorFactory {
    fn new_injector(&self) -> Result<Box<dyn InputInjector>, InjectError> {
        Ok(Box::new(MacInjector::new()?))
    }
}

/// CGEvent 真实注入器：事件经 HID 系统事件源投递。
/// 注意：CGEventSource 非 Send，故逐事件新建（人机输入频率下开销可忽略），
/// 结构体本身保持 Send 以满足注入接缝。
pub struct MacInjector {
    /// 已按住修饰键的 macOS 虚拟键码集合（reset_keys 用）。
    held: HashSet<u16>,
}

impl MacInjector {
    pub fn new() -> Result<Self, InjectError> {
        if !accessibility_granted() {
            return Err(InjectError::PermissionDenied);
        }
        Ok(Self {
            held: HashSet::new(),
        })
    }

    fn source() -> Result<CGEventSource, InjectError> {
        CGEventSource::new(HIDSystemState)
            .map_err(|_| InjectError::Cg("event source create failed".into()))
    }

    fn post(&self, event: &CGEvent) -> Result<(), InjectError> {
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn current_mouse_position() -> CGPoint {
        let Ok(source) = Self::source() else {
            return CGPoint::new(0.0, 0.0);
        };
        match CGEvent::new(source) {
            Ok(e) => e.location(),
            Err(_) => CGPoint::new(0.0, 0.0),
        }
    }
}

impl InputInjector for MacInjector {
    fn mouse_move(&mut self, x: u16, y: u16) -> Result<(), InjectError> {
        let event = CGEvent::new_mouse_event(
            Self::source()?,
            CGEventType::MouseMoved,
            CGPoint::new(f64::from(x), f64::from(y)),
            CGMouseButton::Left,
        )
        .map_err(|_| InjectError::Cg("mouse event create failed".into()))?;
        self.post(&event)
    }

    fn mouse_button(&mut self, btn: MouseButton, down: bool) -> Result<(), InjectError> {
        let (etype, cgbtn) = match (btn, down) {
            (MouseButton::Left, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
            (MouseButton::Left, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (MouseButton::Right, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
            (MouseButton::Right, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
            (MouseButton::Middle, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
            (MouseButton::Middle, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
        };
        let event = CGEvent::new_mouse_event(
            Self::source()?,
            etype,
            Self::current_mouse_position(),
            cgbtn,
        )
        .map_err(|_| InjectError::Cg("mouse button event create failed".into()))?;
        self.post(&event)
    }

    fn mouse_wheel(&mut self, dx: i8, dy: i8) -> Result<(), InjectError> {
        let event = CGEvent::new_scroll_event(
            Self::source()?,
            ScrollEventUnit::PIXEL,
            2,
            i32::from(dy),
            i32::from(dx),
            0,
        )
        .map_err(|_| InjectError::Cg("scroll event create failed".into()))?;
        self.post(&event)
    }

    fn key(&mut self, code: u16, down: bool, _modifiers: u8) -> Result<(), InjectError> {
        let vk = hid_to_mac_vk(code).ok_or(InjectError::UnmappedKey(code))?;
        let event = CGEvent::new_keyboard_event(Self::source()?, vk, down)
            .map_err(|_| InjectError::Cg("keyboard event create failed".into()))?;
        if is_modifier(code) {
            // 修饰键：维护 host 权威按住集合；CGEventCreateKeyboardEvent 对修饰键码
            // 自动产出 FlagsChanged，flags 带全量状态。
            if down {
                if !self.held.insert(vk) {
                    tracing::warn!("rd-input: duplicate modifier down {code:#06x}");
                }
            } else {
                self.held.remove(&vk);
            }
        }
        event.set_flags(flags_for_held(&self.held));
        self.post(&event)
    }

    fn reset_keys(&mut self) -> Result<(), InjectError> {
        let held: Vec<u16> = self.held.drain().collect();
        for vk in held {
            let event = CGEvent::new_keyboard_event(Self::source()?, vk, false)
                .map_err(|_| InjectError::Cg("keyup create failed".into()))?;
            event.set_flags(flags_for_held(&self.held));
            self.post(&event)?;
        }
        Ok(())
    }
}

/// 组合已按住修饰键 → CGEventFlags。
fn flags_for_held(held: &HashSet<u16>) -> CGEventFlags {
    let mut flags = CGEventFlags::CGEventFlagNull;
    for vk in held {
        flags |= vk_flag(*vk);
    }
    flags
}

/// 单修饰键码 → CGEventFlags。
fn vk_flag(vk: u16) -> CGEventFlags {
    match vk {
        0x38 | 0x3C => CGEventFlags::CGEventFlagShift,
        0x3B | 0x3E => CGEventFlags::CGEventFlagControl,
        0x3A | 0x3D => CGEventFlags::CGEventFlagAlternate,
        0x37 | 0x36 => CGEventFlags::CGEventFlagCommand,
        _ => CGEventFlags::CGEventFlagNull,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_combine_held_modifiers() {
        let mut held = HashSet::new();
        held.insert(0x38); // left shift
        held.insert(0x37); // left command
        let flags = flags_for_held(&held);
        assert!(flags.contains(CGEventFlags::CGEventFlagShift));
        assert!(flags.contains(CGEventFlags::CGEventFlagCommand));
        assert!(!flags.contains(CGEventFlags::CGEventFlagControl));
    }

    #[test]
    fn vk_flag_mapping() {
        assert_eq!(vk_flag(0x38), CGEventFlags::CGEventFlagShift);
        assert_eq!(vk_flag(0x3B), CGEventFlags::CGEventFlagControl);
        assert_eq!(vk_flag(0x3D), CGEventFlags::CGEventFlagAlternate);
        assert_eq!(vk_flag(0x36), CGEventFlags::CGEventFlagCommand);
        assert_eq!(vk_flag(0x24), CGEventFlags::CGEventFlagNull);
    }
}
#[cfg(test)]
mod real_smoke {
    use super::*;
    use crate::InputInjector;

    #[test]
    #[ignore = "requires macOS accessibility permission"]
    fn real_inject_smoke() {
        let mut inj = match MacInjector::new() {
            Ok(i) => i,
            Err(InjectError::PermissionDenied) => return, // 无授权:环境限制,非实现回归
            Err(e) => panic!("unexpected init error: {e}"),
        };
        inj.mouse_move(100, 100).expect("mouse move ok");
        inj.mouse_button(MouseButton::Left, true).expect("down ok");
        inj.mouse_button(MouseButton::Left, false).expect("up ok");
        inj.key(0x04, true, 0).expect("key down ok");
        inj.key(0x04, false, 0).expect("key up ok");
        inj.reset_keys().expect("reset ok");
    }
}
