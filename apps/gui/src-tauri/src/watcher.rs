//! 数据目录文件监听（W 波 W1）：notify + debouncer 监听关键数据文件，
//! 防抖归并后向前端发 data-changed{domains}（R2 单监听器消费，定向重载）。
//!
//! 红线：只挂载数据目录与好友簿目录两个非递归 watch + 白名单归类（domains.rs），
//! 禁全目录递归风暴；防抖 ≥500ms；初始化失败返回结构化错误并记日志，
//! GUI 主功能不阻断，降级模式经 data-watch-status 事件可判（R3）。

pub mod domains;

use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecursiveMode, RecommendedWatcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::{info, warn};

use domains::{collect_domains, targets, DataChanged, DataDomain, WatchTargets};

/// 前端定向刷新事件（R2 唯一数据面事件通道）。
pub const DATA_CHANGED_EVENT: &str = "data-changed";
/// 降级可判事件（R3）：{active, reason}，前端据此置降级态。
pub const WATCH_STATUS_EVENT: &str = "data-watch-status";
/// 防抖窗口：≥500ms 合并原子写的连续事件（write tmp + rename 双事件归一）。
pub const DEBOUNCE: Duration = Duration::from_millis(500);

/// 防抖批次：notify-debouncer-mini 的 DebounceEventResult（错误为 notify::Error）。
pub type EventBatch = notify_debouncer_mini::DebounceEventResult;
type SharedDebouncer = Arc<Mutex<Debouncer<RecommendedWatcher>>>;

/// 结构化初始化错误（R3）：stage 定位失败阶段，path 为可选挂载目标。
#[derive(Debug)]
pub struct WatchInitError {
    pub stage: &'static str,
    pub path: Option<std::path::PathBuf>,
    pub message: String,
}

impl std::fmt::Display for WatchInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(p) => write!(
                f,
                "watcher 初始化失败（stage={} path={}）: {message}",
                self.stage,
                p.display(),
                message = self.message
            ),
            None => write!(
                f,
                "watcher 初始化失败（stage={}）: {message}",
                self.stage,
                message = self.message
            ),
        }
    }
}

/// 监听器活体：debouncer 存活即监听生效（managed 持有，进程退出自然回收）。
pub struct WatchHandle {
    _debouncer: SharedDebouncer,
}

/// GUI setup 入口：成功发 data-watch-status{active:true}；失败记结构化日志、
/// 发 degraded 状态并返回结构化错误（调用方只留 stderr 回显，GUI 继续运行）。
pub fn spawn<R: Runtime>(
    app: AppHandle<R>,
    app_data_dir: &Path,
) -> Result<WatchHandle, WatchInitError> {
    match spawn_inner(app_data_dir, DEBOUNCE, Some(app.clone())) {
        Ok((handle, _test_rx)) => {
            emit_status(&app, true, None);
            info!(
                dir = %app_data_dir.display(),
                debounce_ms = DEBOUNCE.as_millis() as u64,
                "数据目录监听已启动（config/profile/chat）"
            );
            Ok(handle)
        }
        Err(e) => {
            tracing::error!(
                stage = e.stage,
                error = %e,
                "数据目录监听启动失败（降级运行，外部写入需手动刷新）"
            );
            emit_status(&app, false, Some(&e.to_string()));
            Err(e)
        }
    }
}

/// 组装 debouncer + 非递归挂载 + 消费线程；app=None 供测试直接消费 rx
/// （Some 分支 rx 归消费线程，返回 None）。
fn spawn_inner<R: Runtime>(
    app_data_dir: &Path,
    debounce: Duration,
    app: Option<AppHandle<R>>,
) -> Result<(WatchHandle, Option<mpsc::Receiver<EventBatch>>), WatchInitError> {
    let targets = targets(app_data_dir);
    let (tx, rx) = mpsc::channel();
    let debouncer: Debouncer<RecommendedWatcher> = new_debouncer(debounce, tx)
        .map_err(|e| WatchInitError { stage: "debouncer", path: None, message: e.to_string() })?;
    let shared: SharedDebouncer = Arc::new(Mutex::new(debouncer));
    watch_dir(&shared, &targets.app_dir)?;
    pre_create_chat_dir(&targets);
    watch_chat_dir(&shared, &targets);
    match app {
        Some(app) => {
            let consumer = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("data-watcher".into())
                .spawn(move || consume(app, consumer, targets, rx))
                .map_err(|e| WatchInitError {
                    stage: "thread",
                    path: None,
                    message: e.to_string(),
                })?;
            Ok((WatchHandle { _debouncer: shared }, None))
        }
        None => Ok((WatchHandle { _debouncer: shared }, Some(rx))),
    }
}

/// 非递归挂载单个目录；错误带 stage+path 便于定位（R3 结构化）。
fn watch_dir(shared: &SharedDebouncer, path: &Path) -> Result<(), WatchInitError> {
    let mut g = lock(shared)?;
    g.watcher()
        .watch(path, RecursiveMode::NonRecursive)
        .map_err(|e| WatchInitError {
            stage: "watch",
            path: Some(path.to_path_buf()),
            message: e.to_string(),
        })
}

/// 启动即预建好友簿目录：CLI 首写好友簿常为「建目录+写文件」同一瞬间，
/// 事后懒挂载存在丢首个事件的窗口；空目录对 p2p-chat 幂等无害。
/// 仅在 app 目录挂载成功后执行（app 目录缺失本身已是初始化失败）。
fn pre_create_chat_dir(targets: &WatchTargets) {
    if let Err(e) = std::fs::create_dir_all(&targets.chat_dir) {
        warn!(error = %e, dir = %targets.chat_dir.display(), "预建好友簿目录失败（chat 域降级）");
    }
}

/// 好友簿目录挂载：挂载失败只降级 chat 域并留告警，不阻断整体启动。
fn watch_chat_dir(shared: &SharedDebouncer, targets: &WatchTargets) {
    if !targets.chat_dir.is_dir() {
        return;
    }
    if let Err(e) = watch_dir(shared, &targets.chat_dir) {
        warn!(error = %e, "好友簿目录监听挂载失败（chat 域降级）");
    }
}

fn lock(
    shared: &SharedDebouncer,
) -> Result<std::sync::MutexGuard<'_, Debouncer<RecommendedWatcher>>, WatchInitError> {
    shared.lock().map_err(|poisoned| WatchInitError {
        stage: "lock",
        path: None,
        message: poisoned.to_string(),
    })
}

/// 消费线程：防抖批次 → 白名单归类 → data-changed；chat 目录后建则懒挂。
/// 通道关闭（debouncer 释放）自然退出；emit 失败留告警不中断（事件面一致性同 events.rs）。
fn consume<R: Runtime>(
    app: AppHandle<R>,
    shared: SharedDebouncer,
    targets: WatchTargets,
    rx: mpsc::Receiver<EventBatch>,
) {
    while let Ok(batch) = rx.recv() {
        match batch {
            Ok(events) => handle_batch(&app, &shared, &targets, &events),
            Err(e) => warn!(error = %e, "文件监听通道错误（保活继续）"),
        }
    }
    info!("文件监听通道关闭，消费线程退出");
}

/// 单批处理：目录懒挂 → 归类去重 → 非空才 emit（无关事件零噪声）。
fn handle_batch<R: Runtime>(
    app: &AppHandle<R>,
    shared: &SharedDebouncer,
    targets: &WatchTargets,
    events: &[DebouncedEvent],
) {
    let paths: Vec<std::path::PathBuf> = events.iter().map(|e| e.path.clone()).collect();
    ensure_chat_dir_watch(shared, targets, &paths);
    let domains = collect_domains(paths);
    if domains.is_empty() {
        return;
    }
    emit_data_changed(app, &domains);
}

/// chat 目录在启动后才创建：看到该目录创建事件即补挂载；目录内文件事件
/// 路径不等于目录本身，天然只在创建瞬间触发（notify 重复 watch 幂等）。
fn ensure_chat_dir_watch(
    shared: &SharedDebouncer,
    targets: &WatchTargets,
    paths: &[std::path::PathBuf],
) {
    if !paths.iter().any(|p| p == &targets.chat_dir) {
        return;
    }
    if let Err(e) = watch_dir(shared, &targets.chat_dir) {
        warn!(error = %e, "好友簿目录懒挂载失败（chat 域降级）");
    } else {
        info!(dir = %targets.chat_dir.display(), "好友簿目录监听已补挂载");
    }
}

fn emit_data_changed<R: Runtime>(app: &AppHandle<R>, domains: &[DataDomain]) {
    let payload = DataChanged {
        domains: domains.iter().map(|d| d.key()).collect(),
    };
    info!(domains = ?payload.domains, "数据目录变更");
    if let Err(e) = app.emit(DATA_CHANGED_EVENT, &payload) {
        warn!(error = %e, "推送 data-changed 失败");
    }
}

#[derive(Clone, Serialize)]
struct WatchStatus<'a> {
    active: bool,
    reason: Option<&'a str>,
}

/// 降级可判（R3）：初始化成败都发状态事件，前端据此展示/记录降级态。
fn emit_status<R: Runtime>(app: &AppHandle<R>, active: bool, reason: Option<&str>) {
    if let Err(e) = app.emit(WATCH_STATUS_EVENT, WatchStatus { active, reason }) {
        warn!(error = %e, "推送 data-watch-status 失败");
    }
}

#[cfg(test)]
mod tests;
