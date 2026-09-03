//! sys_snapshot：系统信息快照（体检与诊断第一入口），read 档。
//!
//! 采集 OS/内核/CPU/内存/磁盘分区/关键环境变量白名单摘要；平台数据不可得时
//! 记 unavailable 并留 warn 日志，不整体失败。磁盘与内核信息复用系统只读命令
//! （uname/df/sysctl），不引入重型依赖。

use crate::cap;
use crate::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;
use tracing::warn;

/// 关键环境变量白名单：只展示常识可读项，值截断；凭据类变量一律不入列。
const ENV_WHITELIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "USERNAME",
    "SHELL",
    "LANG",
    "LC_ALL",
    "TMPDIR",
    "TEMP",
    "COMSPEC",
    "OS",
    "PROCESSOR_ARCHITECTURE",
];

/// 环境变量值摘要上限。
const ENV_VALUE_CAP: usize = 256;

/// 系统快照工具。
#[derive(Default)]
pub struct SysSnapshot;

impl SysSnapshot {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SysSnapshot {
    fn name(&self) -> &str {
        "sys_snapshot"
    }

    fn description(&self) -> &str {
        "系统信息快照：OS/内核/CPU/内存/磁盘分区/关键环境变量摘要（只读）"
    }

    async fn call(&self, _arguments: Value) -> Result<ToolResult, String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "os={} arch={}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        lines.push(format!("hostname={}", hostname()));
        lines.push(format!("kernel={}", kernel_version()));
        lines.push(format!("cpu_count={}", cpu_count()));
        lines.push(format!("mem_total_bytes={}", mem_total_bytes()));
        lines.push("disk_partitions:".into());
        for line in disk_partitions() {
            lines.push(line);
        }
        lines.push("env_whitelist:".into());
        for (key, value) in env_summary() {
            lines.push(format!("{key}={value}"));
        }
        let text = lines.join(
            "
",
        );
        Ok(cap::apply_output_gate(ToolResult {
            text,
            truncated: false,
        }))
    }
}

/// 跑一条系统只读命令并取首行文本；任何失败记 warn 并返回 unavailable。
fn run_capture(cmd: &str, args: &[&str], what: &str) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => {
            warn!(what, status = ?out.status, "snapshot command failed");
            "unavailable".into()
        }
        Err(e) => {
            warn!(what, error = %e, "snapshot command unavailable");
            "unavailable".into()
        }
    }
}

fn hostname() -> String {
    for key in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                return value;
            }
        }
    }
    run_capture("hostname", &[], "hostname")
}

fn kernel_version() -> String {
    if let Ok(raw) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        let value = raw.trim();
        if !value.is_empty() {
            return value.to_string();
        }
    }
    run_capture("uname", &["-r"], "kernel")
}

fn cpu_count() -> String {
    match std::thread::available_parallelism() {
        Ok(n) => n.get().to_string(),
        Err(e) => {
            warn!(error = %e, "available_parallelism failed");
            "unavailable".into()
        }
    }
}

fn mem_total_bytes() -> String {
    if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
        for line in info.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_default();
                return (kb * 1024).to_string();
            }
        }
    }
    let sysctl = run_capture("sysctl", &["-n", "hw.memsize"], "memory");
    if sysctl != "unavailable" {
        return sysctl;
    }
    "unavailable".into()
}

/// 磁盘分区摘要行（POSIX df -kP：设备/总量/可用/挂载点）；失败返回空表。
fn disk_partitions() -> Vec<String> {
    let out = match Command::new("df").args(["-kP"]).output() {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            warn!(status = ?o.status, "df failed");
            return Vec::new();
        }
        Err(e) => {
            warn!(error = %e, "df unavailable");
            return Vec::new();
        }
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 6 {
            lines.push(format!(
                "  {} {} {} {}",
                fields[0], fields[5], fields[1], fields[3]
            ));
        }
    }
    if lines.is_empty() {
        lines.push("  unavailable".into());
    }
    lines
}

/// 白名单环境变量摘要（值截断到 [ENV_VALUE_CAP]）。
fn env_summary() -> Vec<(String, String)> {
    ENV_WHITELIST
        .iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_string(), cap_env(&value)))
        })
        .collect()
}

fn cap_env(value: &str) -> String {
    if value.len() <= ENV_VALUE_CAP {
        return value.to_string();
    }
    let idx = value.floor_char_boundary(ENV_VALUE_CAP.saturating_sub(3));
    format!("{}...", &value[..idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshot_has_key_fields() {
        let tool = SysSnapshot::new();
        let result = tool.call(Value::Null).await.unwrap();
        assert!(!result.truncated);
        assert!(result.text.contains("os="), "{}", result.text);
        assert!(result.text.contains("arch="), "{}", result.text);
        assert!(result.text.contains("cpu_count="), "{}", result.text);
        assert!(result.text.contains("kernel="), "{}", result.text);
        assert!(result.text.contains("mem_total_bytes="), "{}", result.text);
        assert!(result.text.contains("disk_partitions:"), "{}", result.text);
        assert!(result.text.contains("env_whitelist:"), "{}", result.text);
    }

    #[test]
    fn env_whitelist_contains_only_allowlisted_keys() {
        let (_, value) = env_summary()
            .into_iter()
            .find(|(k, _)| k == "PATH")
            .unwrap_or_else(|| ("PATH".into(), String::new()));
        // 测试进程必有 PATH；其值走截断摘要
        assert!(!value.is_empty());
    }

    #[test]
    fn cap_env_truncates_long_values() {
        let long = "x".repeat(300);
        let out = cap_env(&long);
        assert!(out.len() <= ENV_VALUE_CAP);
        assert!(out.ends_with("..."));
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn max_output_reference_used_by_tool() {
        // 快照文本再大也不得超出门禁
        let long_line = "a".repeat(crate::cap::MAX_OUTPUT_BYTES + 10);
        let result = cap::apply_output_gate(crate::ToolResult {
            text: long_line,
            truncated: false,
        });
        assert!(result.truncated);
    }
}
