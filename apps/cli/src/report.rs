//! 节点命令报告事实源与渲染（F8 拆分：lifecycle 管机制，本模块管结构/渲染）。
//! 文本键值行（pid=/peer=/addr=/log=/lanOnly=）与 --json 共用同一 [Report]。

use serde::Serialize;
use serde_json::Value;

use crate::error::CliResult;
use crate::output;
use crate::paths::Paths;

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
    /// lan-only 可见面（F8）：在线报告从控制通道状态回读；离线为 None 不输出。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lan_only: Option<bool>,
    /// start 外联声明全文（F8）：公网逐类列端点，lan-only 单行；status 不携带。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_notice: Option<String>,
}

pub fn placeholder() -> Report {
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
        lan_only: None,
        network_notice: None,
    }
}

/// 补齐 log/data_dir 路径；其余空字段由 placeholder 保证（禁止残留脏值）。
pub fn not_running_report(data_dir: &str, paths: &Paths, mut base: Report) -> Report {
    base.log_path = paths.log().to_string_lossy().into_owned();
    base.data_dir = data_dir.to_string();
    base
}

/// 在线报告：peer/addr/uptime/lanOnly 均回读控制通道状态（单事实源）。
pub fn online_report(paths: &Paths, pid: u32, status: &Value) -> Report {
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
        lan_only: status
            .get("config")
            .and_then(|c| c.get("lanOnly"))
            .and_then(Value::as_bool),
        network_notice: None,
        ..placeholder()
    }
}

/// 文本/JSON 双形态输出（node 域三命令共用；文本键值行供脚本采集）。
pub fn emit(json: bool, report: Report) -> CliResult<()> {
    let text = render_text(&report);
    output::emit(json, &report, &text)
}

pub fn render_text(r: &Report) -> String {
    if r.degraded {
        return format!(
            "节点疑似运行中（pid={} 存活，但控制通道不可达）：{}",
            r.pid.unwrap_or(0),
            r.reason
        );
    }
    if !r.running {
        return match r.stopped {
            Some(true) => format!("已停止节点（pid={}）", r.pid.unwrap_or(0)),
            Some(false) => format!("节点未运行（{}），无需停止", empty_or(r)),
            None => format!("节点未运行（{}）", empty_or(r)),
        };
    }
    running_lines(r).join("\n")
}

fn running_lines(r: &Report) -> Vec<String> {
    let mut lines = vec![match r.already_running {
        Some(true) => format!("节点已在运行（pid={}）", r.pid.unwrap_or(0)),
        Some(false) => format!("节点已启动 pid={}", r.pid.unwrap_or(0)),
        None => format!("节点运行中 pid={}", r.pid.unwrap_or(0)),
    }];
    lines.push(format!("pid={}", r.pid.unwrap_or(0)));
    if let Some(peer) = &r.peer_id {
        lines.push(format!("peer={peer}"));
    }
    lines.extend(r.listen_addrs.iter().map(|a| format!("addr={a}")));
    if let Some(lan_only) = r.lan_only {
        // F8：网络模式键值行，脚本可 grep；lan-only 时 notice 自带单行声明
        lines.push(format!("lanOnly={lan_only}"));
    }
    if let Some(notice) = &r.network_notice {
        lines.push(notice.clone());
    }
    lines.push(format!("log={}", r.log_path));
    if let Some(uptime) = r.uptime_secs {
        lines.push(format!("uptime={uptime}s"));
    }
    lines
}

fn empty_or(r: &Report) -> &str {
    if r.reason.is_empty() {
        "无 pid 文件"
    } else {
        &r.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn running_report() -> Report {
        Report {
            running: true,
            already_running: None,
            stopped: None,
            pid: Some(42),
            peer_id: Some("abc".into()),
            listen_addrs: vec!["127.0.0.1/u1".into()],
            uptime_secs: Some(9),
            log_path: "/tmp/l".into(),
            data_dir: "/tmp/d".into(),
            degraded: false,
            reason: String::new(),
            lan_only: Some(false),
            network_notice: Some("网络模式: 将连接以下公共设施(公网外联)".into()),
        }
    }

    #[test]
    fn report_json_uses_camel_case() {
        let report = running_report();
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["peerId"], json!("abc"));
        assert_eq!(v["listenAddrs"][0], json!("127.0.0.1/u1"));
        assert_eq!(v["uptimeSecs"], json!(3 * 3));
        assert_eq!(v["lanOnly"], json!(false), "lan-only 字段必须出现在 JSON");
        assert!(v.get("stopped").is_none(), "None 字段不输出");
    }

    #[test]
    fn status_text_has_greppable_key_value_lines() {
        let text = render_text(&running_report());
        for key in [
            "pid=42",
            "peer=abc",
            "addr=127.0.0.1/u1",
            "log=/tmp/l",
            "uptime=9s",
            "lanOnly=false",
        ] {
            assert!(text.contains(key), "缺 {key}: {text}");
        }
    }

    #[test]
    fn lan_only_report_declares_lan_mode_without_endpoints() {
        let mut report = running_report();
        report.lan_only = Some(true);
        report.network_notice = Some("网络模式: 仅局域网(lan-only)".into());
        let text = render_text(&report);
        assert!(text.contains("lanOnly=true"));
        assert!(text.contains("仅局域网"));
        assert!(
            !text.contains("43.240.223.138"),
            "lan-only 不列公网端点"
        );
    }

    #[test]
    fn start_and_status_wording_differ() {
        let mut started = running_report();
        started.already_running = Some(false);
        assert!(render_text(&started).starts_with("节点已启动"));
        started.already_running = Some(true);
        assert!(render_text(&started).starts_with("节点已在运行"));
        assert!(render_text(&running_report()).starts_with("节点运行中"));
    }

    #[test]
    fn stop_text_is_idempotent_friendly() {
        let mut stopped = running_report();
        stopped.running = false;
        stopped.stopped = Some(true);
        assert!(render_text(&stopped).contains("已停止节点"));
        stopped.stopped = Some(false);
        stopped.reason = "无 pid 文件".into();
        assert!(render_text(&stopped).contains("节点未运行"));
    }

    #[test]
    fn offline_report_omits_lan_only_field() {
        let report = placeholder();
        let v = serde_json::to_value(&report).unwrap();
        assert!(
            v.get("lanOnly").is_none(),
            "离线无运行时事实，不输出 lanOnly"
        );
    }
}
