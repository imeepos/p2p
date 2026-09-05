//! watcher 单测（R6）：域归类单测在 domains.rs；此处覆盖防抖归并、
//! 白名单过滤、chat 目录懒挂载与降级路径（真实 notify，短防抖窗口）。

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use super::domains::{collect_domains, friends_path, targets, DataDomain};
use super::{spawn_inner, EventBatch};

/// 独立临时目录：测试间互不污染。
fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("p2p-console-watcher-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

/// 测试专用启动：app=None 直接消费 rx；句柄必须由调用方持有到断言结束
/// （debouncer Drop 即停转发线程并断开通道）。
fn start_test(
    dir: &PathBuf,
    debounce: Duration,
) -> (super::WatchHandle, mpsc::Receiver<EventBatch>) {
    match spawn_inner::<tauri::Wry>(dir, debounce, None) {
        Ok((handle, Some(rx))) => (handle, rx),
        Ok((_handle, None)) => unreachable!("app=None 必返回 rx"),
        Err(e) => panic!("挂载失败: {e}"),
    }
}

fn batch_paths(batch: EventBatch) -> Vec<PathBuf> {
    batch
        .expect("监听通道错误")
        .iter()
        .map(|e| e.path.clone())
        .collect()
}

/// 轮询 rx 直到目标域齐备（FSEvents/inotify 延迟不确定，统一超时口径 10s）。
fn recv_domains_until(rx: &mpsc::Receiver<EventBatch>, wanted: &[DataDomain]) -> Vec<DataDomain> {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < Duration::from_secs(10) {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(batch) => {
                seen.extend(collect_domains(batch_paths(batch)));
                if wanted.iter().all(|d| seen.contains(d)) {
                    return seen;
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => panic!("监听通道关闭"),
        }
    }
    panic!("超时未收到目标域 {wanted:?}，实得 {seen:?}");
}

#[test]
fn init_failure_on_missing_dir_is_structured() {
    let missing = temp_dir("missing").join("nope");
    let err = match spawn_inner::<tauri::Wry>(&missing, Duration::from_millis(50), None) {
        Err(e) => e,
        Ok(_) => panic!("目录不存在必须结构化报错"),
    };
    assert_eq!(err.stage, "watch", "失败阶段须可定位");
    assert_eq!(
        err.path.as_deref(),
        Some(missing.as_path()),
        "失败路径须可定位"
    );
    assert!(
        err.to_string().contains("watcher 初始化失败"),
        "错误须可读: {err}"
    );
}

#[test]
fn debounced_write_classifies_domain() {
    let dir = temp_dir("debounce");
    let (_handle, rx) = start_test(&dir, Duration::from_millis(100));
    fs::write(dir.join("gui-config.json"), "{}").expect("写 config");
    let seen = recv_domains_until(&rx, &[DataDomain::Config]);
    assert!(seen.contains(&DataDomain::Config));
}

#[test]
fn rapid_same_file_writes_coalesce_via_debounce() {
    let dir = temp_dir("burst");
    let (_handle, rx) = start_test(&dir, Duration::from_millis(400));
    // 同文件快速连写：防抖按路径归并，首个批次内 config 域恰一次
    for i in 0..3 {
        fs::write(dir.join("gui-config.json"), format!("{{\"i\":{i}}}")).expect("写 config");
    }
    let seen = recv_domains_until(&rx, &[DataDomain::Config]);
    assert_eq!(
        seen.iter().filter(|d| **d == DataDomain::Config).count(),
        1,
        "同文件连写须防抖归一: {seen:?}"
    );
    // 批次消费后无新写入，1.5s 内不得再推 config（无风暴转发）
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        match rx.try_recv() {
            Ok(batch) => {
                let more = collect_domains(batch_paths(batch));
                assert!(
                    !more.contains(&DataDomain::Config),
                    "防抖后不得重复推送: {more:?}"
                );
            }
            Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => panic!("监听通道关闭"),
        }
    }
}

#[test]
fn chat_dir_lazily_watched_when_created_late() {
    let dir = temp_dir("lazy-chat");
    let (_handle, rx) = start_test(&dir, Duration::from_millis(100));
    fs::create_dir_all(dir.join("chat")).expect("后建 chat 目录");
    // 等「目录创建事件 → 懒挂载生效」完成再写文件：建目录与首写同瞬间的
    // 竞态窗口由启动期预建目录消除，此处单测只验证懒挂载机制本身。
    std::thread::sleep(Duration::from_millis(600));
    fs::write(friends_path(&dir), "[]").expect("写好友簿");
    let seen = recv_domains_until(&rx, &[DataDomain::Chat]);
    assert!(
        seen.contains(&DataDomain::Chat),
        "懒挂载后 chat 域事件须送达: {seen:?}"
    );
}

#[test]
fn whitelist_filters_recursive_noise() {
    let dir = temp_dir("noise");
    let (_handle, rx) = start_test(&dir, Duration::from_millis(100));
    fs::create_dir_all(dir.join("p2p-data")).expect("建无关目录");
    fs::write(dir.join("key.seed"), "x").expect("写无关文件");
    fs::write(dir.join("endpoint.json"), "x").expect("写无关文件");
    // 白名单外 2s 内不得产生任何归类（防递归风暴口径）
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        match rx.try_recv() {
            Ok(batch) => {
                let seen = collect_domains(batch_paths(batch));
                assert!(seen.is_empty(), "白名单外事件不得归类: {seen:?}");
            }
            Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => panic!("监听通道关闭"),
        }
    }
    assert_eq!(targets(&dir).chat_dir, dir.join("chat"));
}
