//! rd 输入命令面（M6C §21.4）：crate 根薄壳——cli-parity 门禁按两段路径
//! `rd_input::rd_input_*` 提取命令名，宏查找亦在此模块；实现挂在 RdSlot。

use tauri::State;

use crate::state::AppState;

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
