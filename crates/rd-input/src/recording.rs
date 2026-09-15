//! RecordingInjector：把注入事件记入共享 Vec（E2E/单测断言用），零系统依赖。

use std::sync::{Arc, Mutex};

use crate::{InjectError, InputInjector, MouseButton, Recorded};

/// 记录型注入器：所有调用仅追加 [Recorded] 事件，不产生真实输入。
pub struct RecordingInjector {
    events: Arc<Mutex<Vec<Recorded>>>,
}

impl RecordingInjector {
    pub fn new(events: Arc<Mutex<Vec<Recorded>>>) -> Self {
        Self { events }
    }

    pub fn events(&self) -> Arc<Mutex<Vec<Recorded>>> {
        self.events.clone()
    }
}

impl InputInjector for RecordingInjector {
    fn mouse_move(&mut self, x: u16, y: u16) -> Result<(), InjectError> {
        self.events
            .lock()
            .map(|mut g| g.push(Recorded::MouseMove { x, y }))
            .unwrap_or(());
        Ok(())
    }

    fn mouse_button(&mut self, btn: MouseButton, down: bool) -> Result<(), InjectError> {
        self.events
            .lock()
            .map(|mut g| g.push(Recorded::MouseButton { btn, down }))
            .unwrap_or(());
        Ok(())
    }

    fn mouse_wheel(&mut self, dx: i8, dy: i8) -> Result<(), InjectError> {
        self.events
            .lock()
            .map(|mut g| g.push(Recorded::MouseWheel { dx, dy }))
            .unwrap_or(());
        Ok(())
    }

    fn key(&mut self, code: u16, down: bool, modifiers: u8) -> Result<(), InjectError> {
        self.events
            .lock()
            .map(|mut g| {
                g.push(Recorded::Key {
                    code,
                    down,
                    modifiers,
                })
            })
            .unwrap_or(());
        Ok(())
    }

    fn reset_keys(&mut self) -> Result<(), InjectError> {
        self.events
            .lock()
            .map(|mut g| g.push(Recorded::Reset))
            .unwrap_or(());
        Ok(())
    }
}

/// 记录型注入器工厂（E2E 装配默认）。
#[derive(Clone, Default)]
pub struct RecordingInjectorFactory {
    events: Arc<Mutex<Vec<Recorded>>>,
}

impl RecordingInjectorFactory {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn events(&self) -> Arc<Mutex<Vec<Recorded>>> {
        self.events.clone()
    }
}

impl crate::InjectorFactory for RecordingInjectorFactory {
    fn new_injector(&self) -> Result<Box<dyn InputInjector>, InjectError> {
        Ok(Box::new(RecordingInjector::new(self.events.clone())))
    }
}
