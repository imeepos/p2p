//! 共享内存剪贴板后端：E2E/单测用，宿主可经共享 Arc 模拟外部变更。

use std::sync::{Arc, Mutex};

use crate::{ClipError, ClipboardBackend, ClipboardFactory};

/// 共享状态：Option<String> = 当前剪贴板文本（None = 空）。
pub type SharedClip = Arc<Mutex<Option<String>>>;

/// 内存剪贴板后端：读写共享 Arc；`set_external` 模拟系统侧变更。
pub struct MemoryClipboard {
    inner: SharedClip,
}

impl MemoryClipboard {
    pub fn new(inner: SharedClip) -> Self {
        Self { inner }
    }
}

impl ClipboardBackend for MemoryClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipError> {
        Ok(self.inner.lock().map(|g| g.clone()).unwrap_or(None))
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipError> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| ClipError::Io("lock poisoned".into()))?;
        *g = Some(text.to_string());
        Ok(())
    }
}

/// 内存剪贴板工厂：同一会话内所有后端共享同一 Arc。
#[derive(Clone, Default)]
pub struct MemoryClipboardFactory {
    inner: SharedClip,
}

impl MemoryClipboardFactory {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    /// 共享状态句柄（E2E 断言用）。
    pub fn inner(&self) -> SharedClip {
        self.inner.clone()
    }

    /// 模拟系统侧外部变更（E2E 触发 host→viewer 方向）。
    pub fn set_external(&self, text: &str) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(text.to_string());
    }
}

impl ClipboardFactory for MemoryClipboardFactory {
    fn new_clipboard(&self) -> Result<Box<dyn ClipboardBackend>, ClipError> {
        Ok(Box::new(MemoryClipboard::new(self.inner.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_backend_roundtrip() {
        let f = MemoryClipboardFactory::new();
        let mut backend = f.new_clipboard().unwrap();
        assert_eq!(backend.read_text().unwrap(), None);
        backend.write_text("hello").unwrap();
        assert_eq!(backend.read_text().unwrap(), Some("hello".into()));
        // 工厂共享同一 Arc：新后端读到同一内容
        let mut backend2 = f.new_clipboard().unwrap();
        assert_eq!(backend2.read_text().unwrap(), Some("hello".into()));
    }

    #[test]
    fn external_change_visible() {
        let f = MemoryClipboardFactory::new();
        let mut backend = f.new_clipboard().unwrap();
        f.set_external("from system");
        assert_eq!(backend.read_text().unwrap(), Some("from system".into()));
    }
}
