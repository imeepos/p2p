//! 子进程 spawn 与 stderr 接管（设计 §4.2-5）：stdout/stdin 纯协议流，
//! stderr 逐块写入滚动日志（复用 p2p-log RollingFile，不另造轮子）。

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use p2p_log::RollingFile;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tracing_subscriber::fmt::MakeWriter as _;

use crate::config::{CHILD_LOG_MAX_BYTES, CHILD_LOG_MAX_FILES};

pub struct Subprocess {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
}

/// spawn 桥专属子进程；stdio 三路全接管；cwd 由 jail 模块按 scope 解析（None = 继承）。
/// 失败上抛由会话层审计并断流。
pub fn spawn(argv: &[String], stderr_log: PathBuf, cwd: Option<PathBuf>) -> io::Result<Subprocess> {
    let rolling = RollingFile::new(stderr_log, CHILD_LOG_MAX_BYTES, CHILD_LOG_MAX_FILES)?;
    let mut command = tokio::process::Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 兜底：会话异常路径即使漏杀，句柄落体时内核也会收掉子进程
        .kill_on_drop(true);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let mut child = command.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("child stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("child stdout unavailable"))?;
    let stderr: ChildStderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("child stderr unavailable"))?;
    tokio::spawn(stderr_to_log(stderr, rolling));
    Ok(Subprocess {
        child,
        stdin,
        stdout,
    })
}

async fn stderr_to_log(mut stderr: ChildStderr, log: RollingFile) {
    let mut buf = [0u8; 4096];
    loop {
        match stderr.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                if let Err(err) = write_chunk(&log, &buf[..n]) {
                    tracing::warn!(error = %err, "child stderr log write failed; giving up");
                    break;
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "child stderr read failed");
                break;
            }
        }
    }
}

fn write_chunk(log: &RollingFile, bytes: &[u8]) -> io::Result<()> {
    let mut writer = log.make_writer();
    writer.write_all(bytes)
}

/// 退出阶梯末段：宽限等待退出；超时 SIGKILL。返回可审计的结束描述。
pub(crate) async fn reap(mut child: Child, grace: Duration) -> String {
    match tokio::time::timeout(grace, child.wait()).await {
        Ok(Ok(status)) => format!("exit {status}"),
        Ok(Err(err)) => format!("wait failed: {err}"),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            "killed after grace".to_owned()
        }
    }
}
