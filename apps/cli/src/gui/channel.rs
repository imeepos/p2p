//! GUI 控制通道客户端（GC2）：发现（endpoint.json + pid 探活）→ token → Bearer HTTP。
//! 契约权威：docs/design/gui-control-channel.md §2/§3/§4 与
//! apps/gui/src-tauri/src/control/（本模块只消费，不改 GUI 侧）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::error::CliError;

/// GUI identifier（tauri.conf.json）；app 数据目录 = 系统应用目录下该名字。
const GUI_IDENTIFIER: &str = "com.p2p.console";
/// 单请求上限：截图/录屏收尾服务端可达数秒，30s 覆盖全部原语。
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// 已鉴权的控制通道句柄：一次发现，多次原语调用。
pub struct Channel {
    client: reqwest::Client,
    base_url: String,
    token: String,
}

/// 解析 GUI 数据目录：显式覆盖优先，否则按 OS 约定（与 Tauri app_data_dir 同规则）。
pub fn default_data_dir() -> Result<PathBuf, CliError> {
    #[cfg(target_os = "macos")]
    {
        home().map(|h| h.join("Library/Application Support").join(GUI_IDENTIFIER))
    }
    #[cfg(target_os = "linux")]
    {
        let base = match std::env::var("XDG_DATA_HOME") {
            Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => home()?.join(".local/share"),
        };
        Ok(base.join(GUI_IDENTIFIER))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(|d| PathBuf::from(d).join(GUI_IDENTIFIER))
            .map_err(|_| CliError::Runtime("无法定位 GUI 数据目录（APPDATA 未设置）".into()))
    }
}

fn home() -> Result<PathBuf, CliError> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| CliError::Runtime("无法定位用户主目录（HOME 未设置）".into()))
}

/// 发现 + 鉴权：端点缺失/进程已死/token 缺失均为结构化错误（退出码 1，含启动指引）。
pub fn connect(control_dir: &Path) -> Result<Channel, CliError> {
    let endpoint_file = control_dir.join("endpoint.json");
    let raw = std::fs::read_to_string(&endpoint_file).map_err(|_| {
        CliError::Runtime(format!("未发现 GUI 控制通道端点（{} 不存在）：GUI 未运行——请先启动 GUI（p2p-console）后重试",
            endpoint_file.display()))
    })?;
    let endpoint: Value = serde_json::from_str(raw.trim()).map_err(|e| {
        CliError::Runtime(format!("端点状态文件 {} 非法 JSON: {e}", endpoint_file.display()))
    })?;
    let http = endpoint["http"]
        .as_str()
        .filter(|s| s.starts_with("127.0.0.1:"))
        .ok_or_else(|| CliError::Runtime(format!("端点状态文件缺合法 http 字段: {endpoint}")))?;
    let pid = endpoint["pid"].as_u64().unwrap_or(0);
    if !process_alive(pid) {
        return Err(CliError::Runtime(format!("端点状态文件为残留（进程 pid={pid} 已退出）：GUI 已崩溃或未运行——请先启动 GUI（p2p-console）后重试")));
    }
    let token_path = control_dir.join("token");
    let token = std::fs::read_to_string(&token_path).map_err(|_| {
        CliError::Runtime(format!("控制通道 token 文件缺失（{}）：GUI 未完成初始化——请先启动 GUI（p2p-console）后重试",
            token_path.display()))
    })?.trim().to_string();
    if token.is_empty() {
        return Err(CliError::Runtime(format!("控制通道 token 文件为空（{}）——请先启动 GUI（p2p-console）重新生成", token_path.display())));
    }
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| CliError::Runtime(format!("HTTP 客户端初始化失败: {e}")))?;
    Ok(Channel { client, base_url: format!("http://{http}"), token })
}

/// pid 探活：kill 0；ESRCH=已退出，EPERM=存活但属主不同（仍视为存活）。
fn process_alive(pid: u64) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: kill(2) 以 0 信号做纯探测，无副作用。
    let rc = unsafe { libc::kill(pid as i32, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

impl Channel {
    /// GET 原语，返回 ok=true 的 data 字段。
    pub async fn get(&self, path: &str) -> Result<Value, CliError> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| CliError::Runtime(format!("控制通道请求失败（GET {path}）: {e}")))?;
        unwrap_payload(resp, path).await
    }

    /// POST 原语；body 为 Value::Null 时发送空请求体。
    pub async fn post(&self, path: &str, body: Value) -> Result<Value, CliError> {
        let url = format!("{}{path}", self.base_url);
        let mut req = self.client.post(&url).bearer_auth(&self.token);
        if !body.is_null() {
            req = req.json(&body);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| CliError::Runtime(format!("控制通道请求失败（POST {path}）: {e}")))?;
        unwrap_payload(resp, path).await
    }
}

/// 拆 {ok,data} 或 {ok,error} 包装：失败时透传服务端 code+message（保留权限语义供上层识别）。
async fn unwrap_payload(resp: reqwest::Response, path: &str) -> Result<Value, CliError> {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let payload: Value = serde_json::from_str(&body).map_err(|e| {
        CliError::Runtime(format!("控制通道响应非法（HTTP {status} {path}）: {e}: {body}"))
    })?;
    if payload["ok"] == Value::Bool(true) {
        return Ok(payload["data"].clone());
    }
    let code = payload["error"]["code"].as_str().unwrap_or("UNKNOWN");
    let message = payload["error"]["message"].as_str().unwrap_or("无错误描述");
    Err(CliError::Runtime(format!("[{code}] {message}（HTTP {status}）")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_endpoint(dir: &Path, pid: u64, with_token: bool) {
        std::fs::create_dir_all(dir).unwrap();
        let mut f = std::fs::File::create(dir.join("endpoint.json")).unwrap();
        write!(f, "{{\"http\":\"127.0.0.1:1\",\"pid\":{pid},\"version\":\"0.0.0\",\"startedAtMs\":1,\"tokenFile\":\"control/token\"}}").unwrap();
        if with_token {
            std::fs::File::create(dir.join("token")).unwrap().write_all(b"t").unwrap();
        }
    }

    /// unwrap_err 需要 T: Debug（Channel 非 Debug），改走显式 match 取 CliError。
    fn connect_err(dir: &Path) -> CliError {
        match connect(dir) {
            Err(e) => e,
            Ok(_) => panic!("connect 应失败但成功"),
        }
    }
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gc2_ch_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_endpoint_mentions_startup_hint() {
        let err = connect_err(&scratch("missing"));
        let msg = err.to_string();
        assert!(msg.contains("请先启动 GUI"), "{msg}");
        assert_eq!(err.exit_code(), 1);
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn dead_pid_detected_as_stale_endpoint() {
        let dir = scratch("dead");
        write_endpoint(&dir, u32::MAX as u64 - 1, true);
        let msg = connect_err(&dir).to_string();
        assert!(msg.contains("已退出") && msg.contains("请先启动 GUI"), "{msg}");
    }

    #[test]
    fn missing_token_is_structured_error() {
        let dir = scratch("notok");
        write_endpoint(&dir, std::process::id() as u64, false);
        let msg = connect_err(&dir).to_string();
        assert!(msg.contains("token"), "{msg}");
    }

    #[test]
    fn default_data_dir_matches_identifier() {
        let dir = default_data_dir().unwrap();
        assert!(dir.to_string_lossy().contains(GUI_IDENTIFIER), "{dir:?}");
    }

    #[test]
    fn self_pid_is_alive_and_zero_is_not() {
        assert!(process_alive(std::process::id() as u64));
        assert!(!process_alive(0));
    }
}
