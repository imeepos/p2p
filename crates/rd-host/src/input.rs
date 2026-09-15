//! 控制循环输入分发：把 wire 输入消息落到注入器。
//! 按键状态（[InputDispatch::buttons]）随会话存活，reset 时清零。

use rd_input::{InjectError, InputInjector, MouseButton};

/// 输入分发器：持有注入器与按键状态，逐消息同步。
pub struct InputDispatch {
    injector: Box<dyn InputInjector>,
    buttons: [bool; 3],
}

impl InputDispatch {
    pub fn new(injector: Box<dyn InputInjector>) -> Self {
        Self {
            injector,
            buttons: [false; 3],
        }
    }

    /// 鼠标消息：先移动到位 → 按键 diff → 滚轮（点击落在目标位置）。
    pub fn mouse(&mut self, x: u16, y: u16, mask: u8, wheel_dx: i8, wheel_dy: i8) {
        if let Err(e) = self.injector.mouse_move(x, y) {
            warn_inject("mouse_move", &e);
        }
        let cur = MouseButton::from_mask(mask);
        for (i, down) in cur.into_iter().enumerate() {
            if down != self.buttons[i] {
                if let Err(e) = self.injector.mouse_button(btn_at(i), down) {
                    warn_inject("mouse_button", &e);
                }
                self.buttons[i] = down;
            }
        }
        if wheel_dx != 0 || wheel_dy != 0 {
            if let Err(e) = self.injector.mouse_wheel(wheel_dx, wheel_dy) {
                warn_inject("mouse_wheel", &e);
            }
        }
    }

    /// 键盘消息：未映射键由注入器报错，丢弃+告警（不清流）。
    pub fn key(&mut self, code: u16, down: bool, modifiers: u8) {
        if let Err(e) = self.injector.key(code, down, modifiers) {
            warn_inject("key", &e);
        }
    }

    /// 释放全部按键并清按钮态（viewer 失焦/断线/会话退出 MUST 调用）。
    pub fn reset(&mut self) {
        if let Err(e) = self.injector.reset_keys() {
            warn_inject("reset_keys", &e);
        }
        self.buttons = [false; 3];
    }
}

fn btn_at(i: usize) -> MouseButton {
    match i {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

fn warn_inject(op: &str, e: &InjectError) {
    tracing::warn!("rd-host: {op} inject failed: {e}");
}
