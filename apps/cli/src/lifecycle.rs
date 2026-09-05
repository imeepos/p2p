//! 守护进程生命周期机制：pid 存活探测、控制通道探测、启停信号、现场清理。
//! 命令面（node.rs）只负责子命令解析与输出渲染；本模块输出 Report 事实源。

use std::os::unix::process::CommandExt;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use crate::control;
use crate::error::{CliError, CliResult};
use crate::paths::{remove_file_if_exists, Paths};

const START_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_STEP: Duration = Duration::from_millis(150);

/// 探测/操作结论：文本与 JSON 共用同一事实源。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped: Option<bool>,
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub listen_addrs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,
    pub log_path: String,
    pub data_dir: String,
    /// pid 存活但控制通道不可达。
    pub degraded: bool,
    pub reason: String,
}

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
        Probe::Online { pid, status } => online_report(&paths, pid, status),
        Probe::Degraded { pid, why } => not_running_report(
            data_dir,
            &paths,
            Report {
                running: true,
                pid: Some(pid),
                degraded: true,
                reason: why,
                ..placeholder()
            },
        ),
        Probe::Offline { why } => not_running_report(
            data_dir,
            &paths,
            Report {
                running: false,
                reason: why,
                ..placeholder()
            },
        ),
    }
}

fn placeholder() -> Report {
    Report {
        running: false,
        already_running: None,
        stopped: None,
        pid: None,
        peer_id: None,
        listen_addrs: Vec::new(),
        uptime_secs: None,
        log_path: String::new(),
        data_dir: String::new(),
        degraded: false,
        reason: String::new(),
    }
}

/// 补齐 log/data_dir 路径；其余空字段由 placeholder 保证（禁止残留脏值）。
fn not_running_report(data_dir: &str, paths: &Paths, mut base: Report) -> Report {
    base.log_path = paths.log().to_string_lossy().into_owned();
    base.data_dir = data_dir.to_string();
    base
}

/// start 主体：已运行直接报；清理残留 → 拉守护进程 → 轮询就绪。
pub async fn start_report(data_dir: &str) -> CliResult<Report> {
    let paths = Paths::new(data_dir);
    paths
        .ensure_dir()
        .map_err(|e| CliError::Runtime(format!("创建数据目录失败: {e}")))?;
    if let Probe::Online { pid, status } = probe(&paths).await {
        let mut report = online_report(&paths, pid, status);
        report.already_running = Some(true);
        return Ok(report);
    }
    clean_stale(&paths);
    let child = spawn_daemon(&paths)?;
    wait_ready(&paths, child).await
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
        ..placeholder()
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
            let mut report = online_report(paths, pid, status);
            report.already_running = Some(false);
            return Ok(report);
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

fn online_report(paths: &Paths, pid: u32, status: Value) -> Report {
    Report {
        running: true,
        pid: Some(pid),
        peer_id: status
            .get("peerId")
            .and_then(Value::as_str)
            .map(String::from),
        listen_addrs: status
            .get("listenAddrs")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        uptime_secs: status.get("uptimeSecs").and_then(Value::as_u64),
        log_path: paths.log().to_string_lossy().into_owned(),
        data_dir: paths.root.to_string_lossy().into_owned(),
        ..placeholder()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_json_uses_camel_case() {
        let report = Report {
            running: true,
            already_running: Some(false),
            stopped: None,
            pid: Some(7),
            peer_id: Some("abc".into()),
            listen_addrs: vec!["127.0.0.1/u1".into()],
            uptime_secs: Some(3),
            log_path: "/tmp/l".into(),
            data_dir: "/tmp/d".into(),
            degraded: false,
            reason: String::new(),
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["peerId"], json!("abc"));
        assert_eq!(v["listenAddrs"][0], json!("127.0.0.1/u1"));
        assert_eq!(v["uptimeSecs"], json!(3));
        assert!(v.get("stopped").is_none(), "None 字段不输出");
    }
}
