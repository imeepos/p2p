//! 守护进程生命周期机制：pid 存活探测、控制通道探测、启停信号、现场清理。
//! 命令面（node.rs）只负责子命令解析与输出渲染；本模块输出 Report 事实源。

use std::os::unix::process::CommandExt;
use std::time::Duration;

use serde_json::{json, Value};

use crate::control;
use crate::error::{CliError, CliResult};
use crate::paths::{remove_file_if_exists, Paths};
use crate::report::{self, Report};

const START_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_STEP: Duration = Duration::from_millis(150);

pub enum Probe {
    Online { pid: u32, status: Value },
    Degraded { pid: u32, why: String },
    Offline { why: String },
}

/// 判定顺序：pid 存活 → 控制通道取状态；pid 存活但通道不可达为降级态。
pub async fn probe(paths: &Paths) -> Probe {
    let Some(pid) = read_pid(paths) else {
        return Probe::Offline {
            why: format!("无 pid 文件 {}", paths.pid().display()),
        };
    };
    if !pid_alive(pid as i32) {
        return Probe::Offline {
            why: format!("pid 文件残留但进程 {pid} 已不存在"),
        };
    }
    match control::call(paths, json!({ "op": "status" })).await {
        Ok(status) => Probe::Online { pid, status },
        Err(e) => Probe::Degraded {
            pid,
            why: e.to_string(),
        },
    }
}

/// 在线探测：返回守护进程 pid（在线时）。
pub async fn probe_online(data_dir: &str) -> Option<u32> {
    match probe(&Paths::new(data_dir)).await {
        Probe::Online { pid, .. } => Some(pid),
        _ => None,
    }
}

/// status 事实源。
pub async fn status_report(data_dir: &str) -> Report {
    let paths = Paths::new(data_dir);
    match probe(&paths).await {
        Probe::Online { pid, status } => report::online_report(&paths, pid, &status),
        Probe::Degraded { pid, why } => report::not_running_report(
            data_dir,
            &paths,
            Report {
                running: true,
                pid: Some(pid),
                degraded: true,
                reason: why,
                ..report::placeholder()
            },
        ),
        Probe::Offline { why } => report::not_running_report(
            data_dir,
            &paths,
            Report {
                running: false,
                reason: why,
                ..report::placeholder()
            },
        ),
    }
}

/// start 主体：已运行直接报；清理残留 → 拉守护进程 → 轮询就绪。
/// F8：start 报告携带外联声明（与守护进程同读一份 gui-config.json，声明=实连）。
pub async fn start_report(data_dir: &str) -> CliResult<Report> {
    let paths = Paths::new(data_dir);
    paths
        .ensure_dir()
        .map_err(|e| CliError::Runtime(format!("创建数据目录失败: {e}")))?;
    let config = crate::store::load_config(&paths);
    let notice = crate::notice::notice_for_config(&config).text();
    if let Probe::Online { pid, status } = probe(&paths).await {
        let mut report = report::online_report(&paths, pid, &status);
        report.already_running = Some(true);
        report.network_notice = Some(notice);
        return Ok(report);
    }
    clean_stale(&paths);
    let child = spawn_daemon(&paths)?;
    let mut ready = wait_ready(&paths, child).await?;
    ready.network_notice = Some(notice);
    Ok(ready)
}

/// stop 主体：SIGTERM 优雅等待，超时 SIGKILL 兜底；文件现场必清理。
pub async fn stop_report(data_dir: &str) -> CliResult<Report> {
    let paths = Paths::new(data_dir);
    let pid = read_pid(&paths);
    let (stopped, reported_pid, reason) = match pid {
        Some(pid) if pid_alive(pid as i32) => {
            if terminate(pid, STOP_TIMEOUT) {
                (true, Some(pid), String::new())
            } else {
                return Err(CliError::Runtime(format!(
                    "无法停止进程 {pid}，请人工核查日志 {}",
                    paths.log().display()
                )));
            }
        }
        Some(pid) => (false, None, format!("pid 文件残留且进程 {pid} 已不存在")),
        None => (false, None, "无 pid 文件".into()),
    };
    clean_stale(&paths);
    Ok(Report {
        stopped: Some(stopped),
        pid: reported_pid,
        log_path: paths.log().to_string_lossy().into_owned(),
        data_dir: data_dir.to_string(),
        reason,
        ..report::placeholder()
    })
}

fn spawn_daemon(paths: &Paths) -> CliResult<std::process::Child> {
    // spawn 前冲刷父进程 stdio：stdout 重定向到文件时防缓冲丢写（F9）。
    crate::output::flush_stdio();
    let exe = std::env::current_exe()
        .map_err(|e| CliError::Runtime(format!("定位 p2pctl 可执行文件失败: {e}")))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log())
        .map_err(|e| CliError::Runtime(format!("打开守护进程日志失败: {e}")))?;
    let stderr = log
        .try_clone()
        .map_err(|e| CliError::Runtime(format!("复用日志句柄失败: {e}")))?;
    std::process::Command::new(exe)
        .args(["node", "serve", "--data-dir"])
        .arg(&paths.root)
        .stdout(log)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .map_err(|e| CliError::Runtime(format!("拉起守护进程失败: {e}")))
}

/// 轮询直到控制通道就绪或守护进程退出（退出码与日志路径随错误可观测）。
async fn wait_ready(paths: &Paths, mut child: std::process::Child) -> CliResult<Report> {
    let deadline = tokio::time::Instant::now() + START_TIMEOUT;
    loop {
        if let Some(exit) = child
            .try_wait()
            .map_err(|e| CliError::Runtime(format!("守护进程状态读取失败: {e}")))?
        {
            return Err(CliError::Runtime(format!(
                "节点启动失败（守护进程退出 {exit}），日志 {}",
                paths.log().display()
            )));
        }
        if let Probe::Online { pid, status } = probe(paths).await {
            let mut ready = report::online_report(paths, pid, &status);
            ready.already_running = Some(false);
            return Ok(ready);
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = child.kill();
            return Err(CliError::Runtime(format!(
                "节点启动超时（{START_TIMEOUT:?}），日志 {}",
                paths.log().display()
            )));
        }
        tokio::time::sleep(POLL_STEP).await;
    }
}

/// SIGTERM 优雅等待，超时 SIGKILL 兜底；返回是否真的停掉了进程。
fn terminate(pid: u32, budget: Duration) -> bool {
    let pid = pid as i32;
    if !pid_alive(pid) {
        return false;
    }
    let sent = unsafe { libc::kill(pid, libc::SIGTERM) };
    if sent != 0 {
        eprintln!(
            "p2pctl: SIGTERM 发送失败（pid={pid}）：{}",
            std::io::Error::last_os_error()
        );
    }
    let deadline = tokio::time::Instant::now() + budget;
    while tokio::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(POLL_STEP);
    }
    let killed = unsafe { libc::kill(pid, libc::SIGKILL) } == 0;
    for _ in 0..20 {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(POLL_STEP);
    }
    eprintln!("p2pctl: 进程 {pid} 在 SIGKILL 后仍未退出");
    killed
}

pub fn clean_stale(paths: &Paths) {
    for path in [paths.sock(), paths.pid(), paths.meta()] {
        if let Err(e) = remove_file_if_exists(&path) {
            eprintln!("p2pctl: 清理 {} 失败: {e}", path.display());
        }
    }
}

fn read_pid(paths: &Paths) -> Option<u32> {
    let text = std::fs::read_to_string(paths.pid()).ok()?;
    text.trim().parse().ok()
}

/// kill(pid,0)：0 = 存活；EPERM = 存活但无权限；其余视为不存在。
fn pid_alive(pid: i32) -> bool {
    let sent = unsafe { libc::kill(pid, 0) };
    sent == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
