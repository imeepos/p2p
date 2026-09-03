//! shell_exec：白名单闭集内的进程执行工具（write 档，remote-support-plan.md §5）。
//!
//! 执行语义：argv 数组 + cwd（授权根内）+ timeout（缺省 60s、上限 300s）；
//! stdout/stderr 合并捕获，输出过 256KiB 门禁置 truncated；二进制加标记；
//! 退出码与被杀原因入结果文本头（exit=<code> killed=<reason> duration_ms=<ms>）
//! 并随审计 result_summary 入事件。
//!
//! 执法（§3.4）：执行前经 repair-enforce 双判——红线无条件拒（优先于白名单）→
//! 白名单闭集 + 参数模式（未命中/不匹配拒，不 spawn）→ 风险分级 → scope 门
//! （diag 直接拒）；fix scope 下经审批状态机（[crate::tools::approval]），60s
//! 超时 = 拒绝。审批放行后才 spawn。失败路径分级留日志：spawn 失败/超时 kill/
//! 非零退出/信号终止均显式记录，禁止静默。

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use repair_enforce::approval::{ApprovalVerdict, Approver, Clock};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tracing::warn;

use crate::cap::{self, MAX_OUTPUT_BYTES};
use crate::enforce::{Enforcement, GateOutcome};
use crate::jail::PathJail;
use crate::{Tool, ToolResult};

/// 执行超时缺省（秒）。
const DEFAULT_EXEC_TIMEOUT: u64 = 60;
/// 执行超时上限（秒）：调用方传更大值按上限截断。
const MAX_EXEC_TIMEOUT: u64 = 300;
/// 单流读取上限（上限+1 用于检测超限），防大输出内存炸弹。
const STREAM_CAP: usize = MAX_OUTPUT_BYTES + 1;

/// shell_exec 工具：白名单闭集 + 审批放行后才进程执行。
pub struct ShellExec {
    jail: PathJail,
    enforcement: Enforcement,
    clock: Arc<dyn Clock + Send + Sync>,
    approver: Arc<Mutex<Box<dyn Approver + Send>>>,
}

impl ShellExec {
    pub fn new(
        jail: PathJail,
        enforcement: Enforcement,
        clock: Arc<dyn Clock + Send + Sync>,
        approver: Arc<Mutex<Box<dyn Approver + Send>>>,
    ) -> Self {
        Self {
            jail,
            enforcement,
            clock,
            approver,
        }
    }
}

#[async_trait]
impl Tool for ShellExec {
    fn name(&self) -> &str {
        "shell_exec"
    }

    fn description(&self) -> &str {
        "白名单闭集内执行命令：argv 数组 + 授权根内 cwd + 超时；fix scope 需审批（write 档）"
    }

    async fn call(&self, arguments: Value) -> Result<ToolResult, String> {
        let argv = parse_argv(&arguments)?;
        let timeout_secs = parse_timeout(&arguments);
        let cwd = match arguments.get("cwd").and_then(Value::as_str) {
            Some(raw) => self
                .jail
                .resolve(raw)
                .map_err(|e| format!("shell_exec: cwd: {e}"))?,
            None => self
                .jail
                .first_root()
                .map_err(|e| format!("shell_exec: cwd: {e}"))?,
        };
        match self.enforcement.gate("shell_exec", &arguments) {
            GateOutcome::Deny(reason) => {
                warn!(argv = %argv.join(" "), %reason, "shell_exec denied before spawn");
                Err(format!("shell_exec denied: {reason}"))
            }
            GateOutcome::NeedApproval(_) => {
                require_approval(&self.clock, &self.approver).await?;
                execute(&cwd, &argv, timeout_secs).await
            }
            GateOutcome::Allow(_) => execute(&cwd, &argv, timeout_secs).await,
        }
    }
}

fn parse_argv(arguments: &Value) -> Result<Vec<String>, String> {
    let items = arguments
        .get("argv")
        .and_then(Value::as_array)
        .ok_or_else(|| "shell_exec: missing array param 'argv'".to_string())?;
    let mut argv = Vec::with_capacity(items.len());
    for item in items {
        let s = item
            .as_str()
            .ok_or_else(|| "shell_exec: argv entries must be strings".to_string())?;
        if s.trim().is_empty() {
            return Err("shell_exec: argv entries must not be empty".into());
        }
        argv.push(s.to_string());
    }
    if argv.is_empty() {
        return Err("shell_exec: argv must not be empty".into());
    }
    Ok(argv)
}

fn parse_timeout(arguments: &Value) -> u64 {
    arguments
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_EXEC_TIMEOUT)
        .min(MAX_EXEC_TIMEOUT)
}

/// 审批门：fix scope 下 write/danger 必经状态机；否决/超时即拒（不 spawn）。
async fn require_approval(
    clock: &Arc<dyn Clock + Send + Sync>,
    approver: &Arc<Mutex<Box<dyn Approver + Send>>>,
) -> Result<(), String> {
    match crate::tools::approval::drive_approval(clock.clone(), approver.clone()).await {
        ApprovalVerdict::Approved => Ok(()),
        ApprovalVerdict::Denied => Err("shell_exec: approval denied".into()),
        ApprovalVerdict::Timeout => Err("shell_exec: approval timed out (60s) and denied".into()),
    }
}

/// 进程执行：超时 kill/非零退出/信号终止均落入结果头（exit/killed），不静默。
async fn execute(cwd: &Path, argv: &[String], timeout_secs: u64) -> Result<ToolResult, String> {
    let started = std::time::Instant::now();
    let mut child = spawn_child(cwd, argv)?;
    let out_task = tokio::spawn(read_capped(child.stdout.take()));
    let err_task = tokio::spawn(read_capped(child.stderr.take()));
    let waited =
        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), child.wait()).await;
    let (code, killed): (Option<i32>, Option<String>) = match waited {
        Ok(Ok(status)) => (status.code(), None),
        Ok(Err(e)) => return Err(format!("shell_exec: wait failed: {e}")),
        Err(_elapsed) => {
            warn!(argv = %argv.join(" "), seconds = timeout_secs, "shell_exec timed out; killing");
            if let Err(e) = child.kill().await {
                warn!(error = %e, "shell_exec kill failed");
            }
            let status = child
                .wait()
                .await
                .map_err(|e| format!("shell_exec: wait after kill failed: {e}"))?;
            (status.code(), Some("timeout".into()))
        }
    };
    let mut out = match out_task.await {
        Ok(buf) => buf,
        Err(e) => {
            warn!(error = %e, "shell_exec stdout task failed; treating as empty");
            Vec::new()
        }
    };
    let mut err = match err_task.await {
        Ok(buf) => buf,
        Err(e) => {
            warn!(error = %e, "shell_exec stderr task failed; treating as empty");
            Vec::new()
        }
    };
    let over = out.len() > MAX_OUTPUT_BYTES || err.len() > MAX_OUTPUT_BYTES;
    out.truncate(MAX_OUTPUT_BYTES);
    err.truncate(MAX_OUTPUT_BYTES);
    let mut merged = out;
    merged.extend_from_slice(&err);
    record_exit_signal(argv, code);
    let duration_ms = started.elapsed().as_millis() as u64;
    Ok(cap::apply_output_gate(ToolResult {
        text: result_text(code, killed.as_deref(), &merged, duration_ms),
        truncated: over,
    }))
}

fn spawn_child(cwd: &Path, argv: &[String]) -> Result<Child, String> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd.spawn().map_err(|e| {
        warn!(argv = %argv.join(" "), error = %e, "shell_exec spawn failed");
        format!("shell_exec: spawn failed: {e}")
    })
}

/// 按上限边界读流（防内存炸弹）；读满上限即返回，由门禁截断标记。
async fn read_capped<S>(stream: Option<S>) -> Vec<u8>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let Some(s) = stream else {
        return Vec::new();
    };
    let mut buf = Vec::new();
    let _ = s.take(STREAM_CAP as u64).read_to_end(&mut buf).await;
    buf
}

fn record_exit_signal(argv: &[String], code: Option<i32>) {
    match code {
        Some(0) => {}
        Some(c) => warn!(argv = %argv.join(" "), exit = c, "shell_exec exited non-zero"),
        None => warn!(argv = %argv.join(" "), "shell_exec terminated by signal"),
    }
}

fn result_text(code: Option<i32>, killed: Option<&str>, body: &[u8], duration_ms: u64) -> String {
    let exit = code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".into());
    let kill = killed.unwrap_or("none");
    let mut text = String::new();
    if body.contains(&0) {
        text.push_str("binary=1\n");
    }
    text.push_str(&format!(
        "exit={exit} killed={kill} duration_ms={duration_ms}\n"
    ));
    text.push_str(&String::from_utf8_lossy(body));
    text
}
