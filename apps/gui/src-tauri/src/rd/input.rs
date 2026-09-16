//! rd 输入命令面（M6C）：GUI canvas 鼠标/键盘事件 → 活跃 viewer 会话控制通道。
//!
//! 无活跃 viewer 会话时返回 false（幂等护栏，不报错——审批等待/断连瞬间不炸 UI）；
//! 发送失败同样以 false 呈现并记 warn（失败路径可观测）。

use tauri::State;

use super::RdSlot;
use crate::state::AppState;

impl RdSlot {
    pub(super) async fn input_mouse(
        &self,
        x: u16,
        y: u16,
        buttons: u8,
        wheel_dx: i8,
        wheel_dy: i8,
    ) -> bool {
        // Arc 克隆后即刻释放槽位锁：网络 await 不持锁，close/status 不被写半阻塞。
        let ctl = {
            let g = self.viewer.lock().await;
            g.as_ref().map(|s| s.control())
        };
        match ctl {
            Some(ctl) => ctl.mouse(x, y, buttons, wheel_dx, wheel_dy).await.is_ok(),
            None => false,
        }
    }

    pub(super) async fn input_key(&self, code: u16, down: bool, modifiers: u8) -> bool {
        let ctl = {
            let g = self.viewer.lock().await;
            g.as_ref().map(|s| s.control())
        };
        match ctl {
            Some(ctl) => ctl.key(code, down, modifiers).await.is_ok(),
            None => false,
        }
    }

    pub(super) async fn input_key_reset(&self) -> bool {
        let ctl = {
            let g = self.viewer.lock().await;
            g.as_ref().map(|s| s.control())
        };
        match ctl {
            Some(ctl) => ctl.key_reset().await.is_ok(),
            None => false,
        }
    }
}

#[tauri::command]
pub async fn rd_input_mouse(
    state: State<'_, AppState>,
    x: u16,
    y: u16,
    buttons: u8,
    wheel_dx: i8,
    wheel_dy: i8,
) -> Result<bool, String> {
    Ok(state
        .rd()
        .input_mouse(x, y, buttons, wheel_dx, wheel_dy)
        .await)
}

#[tauri::command]
pub async fn rd_input_key(
    state: State<'_, AppState>,
    code: u16,
    down: bool,
    modifiers: u8,
) -> Result<bool, String> {
    Ok(state.rd().input_key(code, down, modifiers).await)
}

#[tauri::command]
pub async fn rd_input_key_reset(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.rd().input_key_reset().await)
}
