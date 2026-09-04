//! GC1 本地控制通道（契约见 docs/design/gui-control-channel.md）：
//! 仅绑 127.0.0.1 的 HTTP JSON 通道 + token 鉴权，供 CLI 截图/录屏/导航/受限 invoke。
//!
//! 红线：不向 generate_handler! 新增命令；通道失败只留可观测日志，不阻塞 GUI 主功能。

pub mod capture;
pub mod handlers;
pub mod invoke_allow;
pub mod paths;
pub mod record;
pub mod server;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tauri::{AppHandle, Listener, Manager, Runtime, WebviewWindow};

use crate::control::capture::FrameSource;

/// navigate 合法路由名（与 App.tsx / menu.def.ts 对齐，"/" 记作 dashboard）。
pub const ROUTES: [&str; 8] = [
    "dashboard",
    "peers",
    "discovery",
    "relay",
    "chat",
    "events",
    "settings",
    "diagnostics",
];

/// 通道运行期共享态：server 线程 / 路由上报监听 / 退出收尾共同持有。
pub struct ControlCtx<R: Runtime> {
    pub app: AppHandle<R>,
    pub token: String,
    pub base_dir: PathBuf,
    pub started_at: Instant,
    pub frame: Arc<dyn FrameSource>,
    pub stop: AtomicBool,
    pub addr: Mutex<Option<SocketAddr>>,
    pub route: Mutex<String>,
    pub record: Mutex<Option<record::RecordSession>>,
}

impl<R: Runtime> ControlCtx<R> {
    pub fn main_window(&self) -> Option<WebviewWindow<R>> {
        self.app.get_webview_window("main")
    }

    /// 当前路由；锁异常降级为 unknown 并留错误日志（禁止 panic 波及 GUI 主线程）。
    pub fn current_route(&self) -> String {
        match self.route.lock() {
            Ok(g) => g.clone(),
            Err(e) => {
                tracing::error!("control: route 锁中毒: {e}");
                "unknown".to_string()
            }
        }
    }
}

/// 通道句柄：GUI 退出（RunEvent::Exit）时调用 shutdown。
pub struct ControlHandle<R: Runtime> {
    pub ctx: Arc<ControlCtx<R>>,
}

impl<R: Runtime> ControlHandle<R> {
    /// 幂等收尾：停录屏 → 置停止位并唤醒 recv → 摘端点状态文件。失败留日志不 panic。
    pub fn shutdown(&self) {
        if let Ok(mut slot) = self.ctx.record.lock() {
            if let Some(session) = slot.take() {
                let stats = session.finalize();
                if stats.bytes == 0 {
                    tracing::error!("control: 退出时录屏收尾未产出文件 {}", stats.path);
                }
            }
        }
        self.ctx.stop.store(true, Ordering::SeqCst);
        if let Ok(addr) = self.ctx.addr.lock() {
            if let Some(addr) = *addr {
                wake_recv(addr);
            }
        }
        paths::remove_endpoint(&paths::control_dir(&self.ctx.base_dir));
        tracing::info!("control: 通道已关闭");
    }
}

/// 发一个无害请求唤醒阻塞中的 server.recv()，使其看到停止位。
fn wake_recv(addr: SocketAddr) {
    use std::io::{Read, Write};
    let _ = std::net::TcpStream::connect(addr).and_then(|mut s| {
        s.write_all(b"GET /__wake HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
        let mut buf = [0u8; 16];
        let _ = s.read(&mut buf);
        Ok(())
    });
}

/// 生产入口：GUI setup 调用。返回 Err 时调用方必须留显式日志，GUI 继续运行。
pub fn spawn<R: Runtime>(
    app: &AppHandle<R>,
    data_dir: &Path,
) -> Result<ControlHandle<R>, String> {
    let window = app.get_webview_window("main");
    let frame: Arc<dyn FrameSource> = Arc::new(capture::RealFrameSource::new(window));
    let token = paths::load_or_create_token(&paths::control_dir(data_dir))?;
    start(app.clone(), data_dir.to_path_buf(), token, frame)
}

/// 测试入口：注入合成帧源与指定 base_dir（端口仍走 listen() 策略）。
#[doc(hidden)]
pub fn spawn_for_test<R: Runtime>(
    app: &AppHandle<R>,
    base_dir: PathBuf,
    frame: Arc<dyn FrameSource>,
) -> Result<ControlHandle<R>, String> {
    let token = paths::load_or_create_token(&paths::control_dir(&base_dir))?;
    start(app.clone(), base_dir, token, frame)
}

fn start<R: Runtime>(
    app: AppHandle<R>,
    base_dir: PathBuf,
    token: String,
    frame: Arc<dyn FrameSource>,
) -> Result<ControlHandle<R>, String> {
    let ctx = Arc::new(ControlCtx {
        app,
        token,
        base_dir,
        frame,
        started_at: Instant::now(),
        stop: AtomicBool::new(false),
        addr: Mutex::new(None),
        route: Mutex::new("dashboard".to_string()),
        record: Mutex::new(None),
    });
    install_route_listener(&ctx);
    let (server, addr) = server::listen()?;
    if let Ok(mut g) = ctx.addr.lock() {
        *g = Some(addr);
    }
    paths::write_endpoint(
        &paths::control_dir(&ctx.base_dir),
        &paths::EndpointInfo {
            http: addr.to_string(),
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at_ms: now_ms(),
            token_file: "control/token".to_string(),
        },
    )?;
    server::spawn_thread(ctx.clone(), server);
    tracing::info!("control: 通道已启动 http://{addr}");
    Ok(ControlHandle { ctx })
}

/// 前端 hashchange → control-route 事件 → 共享路由态（health 的 route 字段）。
fn install_route_listener<R: Runtime>(ctx: &Arc<ControlCtx<R>>) {
    let state = Arc::clone(ctx);
    ctx.app.listen_any("control-route", move |event| {
        let payload: Value = match serde_json::from_str(event.payload()) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("control: 路由上报载荷非法 JSON: {e}");
                return;
            }
        };
        if let Some(route) = payload.get("route").and_then(Value::as_str) {
            match state.route.lock() {
                Ok(mut g) => *g = route.to_string(),
                Err(e) => tracing::error!("control: route 锁中毒: {e}"),
            }
        }
    });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
