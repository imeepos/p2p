//! GC1 控制通道集成测试（R5 验收核心）：
//! mock runtime + 合成帧源，全链路走真实 HTTP（127.0.0.1）。
//! 覆盖：无/错 token 拒绝；health 合法 JSON；screenshot 非空 PNG；
//! record start/stop GIF；navigate 路由切换；未授权 invoke 拒绝；数据文件可发现。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::test::MockRuntime;
use tauri::{App, Manager};

use p2p_console::control::capture::{FrameSource, SyntheticFrameSource};
use p2p_console::control::ControlHandle;
use p2p_console::state::AppState;

static SEQ: AtomicU32 = AtomicU32::new(0);

struct TestEnv {
    _app: App<MockRuntime>,
    control: ControlHandle<MockRuntime>,
    base_dir: PathBuf,
    addr: String,
    token: String,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.control.shutdown();
    }
}

fn setup(tag: &str) -> TestEnv {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let base_dir = std::env::temp_dir().join(format!("gc1_ctl_{tag}_{}_{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&base_dir);
    std::fs::create_dir_all(&base_dir).expect("创建测试目录");
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(base_dir.join("appdata")));
    let frame: Arc<dyn FrameSource> = Arc::new(SyntheticFrameSource::new());
    let control = p2p_console::control::spawn_for_test(&handle, base_dir.clone(), frame)
        .expect("控制通道启动");
    let endpoint: Value = serde_json::from_str(
        &std::fs::read_to_string(base_dir.join("control/endpoint.json")).expect("endpoint.json 可发现"),
    )
    .expect("endpoint.json 合法 JSON");
    let addr = endpoint["http"].as_str().expect("http 字段").to_string();
    let token = std::fs::read_to_string(base_dir.join("control/token"))
        .expect("token 可发现")
        .trim()
        .to_string();
    TestEnv { _app: app, control, base_dir, addr, token }
}

/// 裸 TCP HTTP 客户端：零额外依赖，验证真实 HTTP 行为。
fn call(env: &TestEnv, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (u16, Value) {
    let mut stream = TcpStream::connect(&env.addr).expect("连接控制通道");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("设置读超时");
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n",
        host = env.addr,
        len = payload.len(),
    );
    if let Some(t) = token {
        req.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).expect("写请求头");
    if !payload.is_empty() {
        stream.write_all(payload.as_bytes()).expect("写请求体");
    }
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("读响应");
    let (head, body) = split_head(&raw);
    let status = head_text(head)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let json = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(body).unwrap_or_else(|e| {
            panic!("响应体非法 JSON: {e}: {}", String::from_utf8_lossy(body))
        })
    };
    (status, json)
}

fn head_text(head: &[u8]) -> String {
    String::from_utf8_lossy(head).to_string()
}

fn split_head(raw: &[u8]) -> (&[u8], &[u8]) {
    if raw.len() >= 4 {
        for i in 0..=raw.len() - 4 {
            if &raw[i..i + 4] == b"\r\n\r\n" {
                return (&raw[..i], &raw[i + 4..]);
            }
        }
    }
    (raw, &[])
}

#[test]
fn missing_token_rejected() {
    let env = setup("noauth");
    let (code, body) = call(&env, "GET", "/health", None, None);
    assert_eq!(code, 401, "无 token 必须 401: {body}");
    assert_eq!(body["error"]["code"], "UNAUTHORIZED");
}

#[test]
fn wrong_token_rejected() {
    let env = setup("badauth");
    let (code, body) = call(&env, "GET", "/health", Some(&"0".repeat(64)), None);
    assert_eq!(code, 401, "错 token 必须 401: {body}");
    assert_eq!(body["error"]["code"], "UNAUTHORIZED");
}

#[test]
fn health_returns_valid_json() {
    let env = setup("health");
    let (code, body) = call(&env, "GET", "/health", Some(&env.token), None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["ok"], true, "{body}");
    let data = &body["data"];
    assert!(data["version"].is_string(), "version 必须是字符串: {data}");
    assert!(data["title"].is_string(), "title 必须是字符串: {data}");
    assert_eq!(data["route"], "dashboard", "初始路由应为 dashboard: {data}");
    assert!(data["pid"].is_u64(), "pid 必须是数值: {data}");
    assert!(data["uptimeMs"].is_u64(), "uptimeMs 必须是数值: {data}");
    assert_eq!(data["recording"], false, "初始未录屏: {data}");
}

#[test]
fn screenshot_writes_nonempty_png() {
    let env = setup("shot");
    let out = env.base_dir.join("shot/window.png");
    let (code, body) = call(
        &env,
        "POST",
        "/screenshot",
        Some(&env.token),
        Some(json!({ "path": out.display().to_string() })),
    );
    assert_eq!(code, 200, "screenshot 应成功: {body}");
    let bytes = std::fs::read(&out).expect("PNG 文件必须存在");
    assert!(!bytes.is_empty(), "禁止空文件");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "必须是 PNG magic");
    let be32 = |o: usize| {
        u32::from_be_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
    };
    assert!(be32(16) > 0 && be32(20) > 0, "IHDR 尺寸必须为正");
    assert_eq!(
        body["data"]["bytes"].as_u64(),
        Some(bytes.len() as u64),
        "bytes 字段与实际文件一致"
    );
}

#[test]
fn screenshot_rejects_bad_requests_without_file() {
    let env = setup("shot-bad");
    let (code, body) = call(
        &env,
        "POST",
        "/screenshot",
        Some(&env.token),
        Some(json!({ "path": "relative/x.png" })),
    );
    assert_eq!(code, 400, "相对路径必须拒绝: {body}");
    assert_eq!(body["error"]["code"], "INVALID_REQUEST");
    let (code, body) = call(&env, "POST", "/screenshot", Some(&env.token), Some(json!({})));
    assert_eq!(code, 400, "缺 path 必须拒绝: {body}");
    let out = PathBuf::from("/gc1-forbidden-subdir/x.png");
    let (code, body) = call(
        &env,
        "POST",
        "/screenshot",
        Some(&env.token),
        Some(json!({ "path": out.display().to_string() })),
    );
    assert_ne!(code, 200, "不可写目录必须结构化报错: {body}");
    assert!(body["error"]["code"].is_string(), "必须有错误码: {body}");
    assert!(!out.exists(), "失败路径禁止产出文件");
}

#[test]
fn record_start_stop_produces_gif() {
    let env = setup("rec");
    let out = env.base_dir.join("rec/out.gif");
    let (code, body) = call(
        &env,
        "POST",
        "/record/start",
        Some(&env.token),
        Some(json!({ "path": out.display().to_string(), "intervalMs": 200 })),
    );
    assert_eq!(code, 200, "record start 应成功: {body}");
    let (code, body) = call(
        &env,
        "POST",
        "/record/start",
        Some(&env.token),
        Some(json!({ "path": out.display().to_string() })),
    );
    assert_eq!(code, 409, "重复 start 必须 409: {body}");
    assert_eq!(body["error"]["code"], "RECORD_CONFLICT");
    let (_, health) = call(&env, "GET", "/health", Some(&env.token), None);
    assert_eq!(health["data"]["recording"], true, "录屏中 health 应如实反映");
    let (code, body) = call(&env, "POST", "/record/stop", Some(&env.token), None);
    assert_eq!(code, 200, "record stop 应成功: {body}");
    assert!(body["data"]["frames"].as_u64().unwrap_or(0) >= 1, "至少 1 帧: {body}");
    let bytes = std::fs::read(&out).expect("GIF 文件必须存在");
    assert!(bytes.len() > 6 && &bytes[..3] == b"GIF", "必须是 GIF magic");
    assert_eq!(
        body["data"]["bytes"].as_u64(),
        Some(bytes.len() as u64),
        "bytes 字段与实际文件一致"
    );
    let (code, body) = call(&env, "POST", "/record/stop", Some(&env.token), None);
    assert_eq!(code, 409, "无录屏时 stop 必须 409: {body}");
    assert_eq!(body["error"]["code"], "RECORD_NOT_ACTIVE");
}

#[test]
fn navigate_switches_route_and_rejects_unknown() {
    let env = setup("nav");
    let (code, body) = call(
        &env,
        "POST",
        "/navigate",
        Some(&env.token),
        Some(json!({ "route": "settings" })),
    );
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["data"]["path"], "#/settings", "{body}");
    let (_, health) = call(&env, "GET", "/health", Some(&env.token), None);
    assert_eq!(health["data"]["route"], "settings", "navigate 后 health 必须反映新路由");
    let (code, body) = call(
        &env,
        "POST",
        "/navigate",
        Some(&env.token),
        Some(json!({ "route": "no-such" })),
    );
    assert_eq!(code, 400, "未知路由必须拒绝: {body}");
    assert_eq!(body["error"]["code"], "INVALID_ROUTE");
    let (code, _) = call(
        &env,
        "POST",
        "/navigate",
        Some(&env.token),
        Some(json!({ "route": "/chat" })),
    );
    assert_eq!(code, 200, "带斜杠路由名应归一化");
    let (_, health) = call(&env, "GET", "/health", Some(&env.token), None);
    assert_eq!(health["data"]["route"], "chat", "{health}");
}

#[test]
fn invoke_whitelist_forward_and_reject() {
    let env = setup("inv");
    let (code, body) = call(
        &env,
        "POST",
        "/invoke",
        Some(&env.token),
        Some(json!({ "command": "node_status" })),
    );
    assert_eq!(code, 200, "白名单命令应转发成功: {body}");
    assert!(body["data"]["result"].is_object(), "node_status 返回对象: {body}");
    let (code, body) = call(
        &env,
        "POST",
        "/invoke",
        Some(&env.token),
        Some(json!({ "command": "config_save" })),
    );
    assert_eq!(code, 403, "写命令必须在白名单外被拒: {body}");
    assert_eq!(body["error"]["code"], "INVOKE_FORBIDDEN");
    let (code, body) = call(
        &env,
        "POST",
        "/invoke",
        Some(&env.token),
        Some(json!({ "command": "std::process::exit" })),
    );
    assert_eq!(code, 403, "任意命令名必须 403: {body}");
    assert_eq!(body["error"]["code"], "INVOKE_FORBIDDEN");
}

#[test]
fn token_file_0600_and_endpoint_discoverable() {
    let env = setup("perm");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(env.base_dir.join("control/token"))
            .expect("token 文件存在")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "token 文件权限必须 600，实际 {:o}", mode);
    }
    let endpoint: Value = serde_json::from_str(
        &std::fs::read_to_string(env.base_dir.join("control/endpoint.json")).expect("endpoint 存在"),
    )
    .expect("endpoint JSON");
    assert!(
        endpoint["http"].as_str().unwrap_or("").starts_with("127.0.0.1:"),
        "只允许回环绑定: {endpoint}"
    );
    assert_eq!(endpoint["pid"].as_u64(), Some(u64::from(std::process::id())));
    assert!(endpoint["version"].is_string(), "{endpoint}");
    assert_eq!(endpoint["tokenFile"], "control/token", "{endpoint}");
}

#[test]
fn unknown_endpoint_404_and_method_mismatch_405() {
    let env = setup("404");
    let (code, _) = call(&env, "GET", "/nope", Some(&env.token), None);
    assert_eq!(code, 404);
    let (code, _) = call(&env, "GET", "/screenshot", Some(&env.token), None);
    assert_eq!(code, 405);
}
