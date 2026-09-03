//! panic 钩子：panic 信息写入日志（error 级）并回显 stderr。
//!
//! 链式安装：先取默认钩子，新钩子记日志后调用默认钩子，
//! 保证 stderr 回显行为与默认一致、不重复打印。

use std::panic::PanicHookInfo;
use std::sync::Once;

static INSTALL: Once = Once::new();

/// 安装 panic 钩子（幂等：进程内只安装一次）。
pub fn install() {
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let text = describe(info);
            tracing::error!(target: "p2p_panic", panic = %text, "进程 panic");
            previous(info);
        }));
    });
}

/// 把 panic 载荷与位置压成一行可读文本（供日志与测试）。
pub fn describe(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Box<dyn Any>（非文本 panic 载荷）".to_string()
    };
    match info.location() {
        Some(loc) => format!("{message} (at {}:{})", loc.file(), loc.line()),
        None => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// 钩子是进程级全局：描述类测试串行操作钩子，避免互相干扰。
    static HOOK_LOCK: Mutex<()> = Mutex::new(());

    /// 用临时捕获钩子拿真实 PanicHookInfo，跑完恢复原钩子。
    fn describe_ofpanic<F: FnOnce() + std::panic::UnwindSafe>(f: F) -> String {
        let _guard = HOOK_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let captured = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&captured);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            *sink.lock().unwrap() = describe(info);
        }));
        let _ = std::panic::catch_unwind(f);
        drop(std::panic::take_hook());
        std::panic::set_hook(previous);
        let text = captured.lock().unwrap().clone();
        text
    }

    #[test]
    fn describe_includes_message_and_location() {
        let text = describe_ofpanic(|| panic!("boom-marker"));
        assert!(text.contains("boom-marker"), "载荷文本必须保留: {text}");
        assert!(text.contains("panic.rs:"), "必须带位置: {text}");
    }

    #[test]
    fn describe_handles_non_text_payload() {
        let text = describe_ofpanic(|| std::panic::panic_any(42i32));
        assert!(
            text.contains("Box<dyn Any>"),
            "非文本载荷要有可读占位: {text}"
        );
    }

    #[test]
    fn install_is_idempotent_and_never_panics() {
        install();
        install();
    }
}
