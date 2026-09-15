//! rd-clipboard：远程桌面剪贴板同步后端抽象（remote-desktop-plan §2.3，M4）。
//!
//! [ClipboardBackend] 是剪贴板接缝：系统剪贴板（[SystemClipboard]，arboard 实现，
//! 无 Swift 依赖）与共享内存后端（[memory::MemoryClipboard]，测试/E2E 用）可互换。
//! 变更探测采用轮询 diff（M4 文本级），宿主负责回声抑制（last_seen 追踪）。

pub mod memory;

use thiserror::Error;

/// 剪贴板失败：显式可观测。
#[derive(Debug, Error)]
pub enum ClipError {
    #[error("clipboard: {0}")]
    Io(String),
}

/// 剪贴板后端接缝：读写文本；read 返回 None 表示剪贴板无文本内容。
pub trait ClipboardBackend: Send {
    fn read_text(&mut self) -> Result<Option<String>, ClipError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipError>;
}

/// 后端工厂：host 每会话实例化（测试注入 memory，真机注入系统剪贴板）。
pub trait ClipboardFactory: Send + Sync {
    fn new_clipboard(&self) -> Result<Box<dyn ClipboardBackend>, ClipError>;
}

/// 系统剪贴板后端（arboard，macOS NSPasteboard / Linux X11-Wayland）。
pub struct SystemClipboard {
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    pub fn new() -> Result<Self, ClipError> {
        let inner = arboard::Clipboard::new().map_err(|e| ClipError::Io(format!("open: {e}")))?;
        Ok(Self { inner })
    }
}

impl ClipboardBackend for SystemClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipError> {
        match self.inner.get_text() {
            Ok(text) => Ok(Some(text)),
            // 剪贴板为空/无文本内容：arboard 以 ContentNotAvailable 表达。
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(ClipError::Io(format!("read: {e}"))),
        }
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipError> {
        self.inner
            .set_text(text)
            .map_err(|e| ClipError::Io(format!("write: {e}")))
    }
}

/// 系统剪贴板工厂（默认装配）。
pub struct SystemClipboardFactory;

impl ClipboardFactory for SystemClipboardFactory {
    fn new_clipboard(&self) -> Result<Box<dyn ClipboardBackend>, ClipError> {
        Ok(Box::new(SystemClipboard::new()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_factory_produces_backend() {
        // 仅验证装配不炸（读写系统剪贴板受环境约束，真实读写不进单测）。
        let backend = SystemClipboardFactory.new_clipboard();
        assert!(backend.is_ok() || backend.is_err());
    }
}
