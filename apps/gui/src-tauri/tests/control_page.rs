//! GC3 页面语义协议集成测试：describe 回包 / action 执行回包 / 超时 / 拒绝路径。
//! mock runtime 无真实 webview：以 AppHandle.emit 模拟前端 control-page-reply
//! 回执（与生产 page-bridge 同一事件名与载荷形状），HTTP 全链路走真实 127.0.0.1。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::test::MockRuntime;
use tauri::{App, AppHandle, Emitter, Manager};

use p2p_console::control::capture::{FrameSource, SyntheticFrameSource};
use p2p_console::control::{ControlHandle, PAGE_REPLY_EVENT};
use p2p_console::state::AppState;

static SEQ: AtomicU32 = AtomicU32::new(0);

struct TestEnv {
    _app: App<MockRuntime>,
    control: ControlHandle<MockRuntime>,
    addr: String,
    token: String,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.control.shutdown();
    }
}

impl TestEnv {
    fn app_handle(&self) -> AppHandle<MockRuntime> {
        self._app.handle().clone()
    }
}

fn setup(tag: &str) -> TestEnv {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let base_dir = std::env::temp_dir().join(format!("gc3_page_{tag}_{}_{}", std::process::id(), n));
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
    TestEnv { _app: app, control, addr, token }
}

/// 裸 TCP HTTP 客户端（与 control_channel.rs 同款，本文件自包含）。
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
    let status = String::from_utf8_lossy(head)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let json = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(body).expect("响应体合法 JSON")
    };
    (status, json)
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

/// 模拟前端回执：延迟后以生产事件名发 control-page-reply（requestId 关联）。
fn simulate_reply(handle: &AppHandle<MockRuntime>, payload: Value) {
    let handle = handle.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        handle.emit(PAGE_REPLY_EVENT, payload).expect("emit 页面回执");
    });
}

fn ok_reply(request_id: &str, data: Value) -> Value {
    json!({ "requestId": request_id, "ok": true, "data": data })
}

fn err_reply(request_id: &str, code: &str) -> Value {
    json!({ "requestId": request_id, "ok": false, "error": { "code": code, "message": "reason" } })
}

#[test]
fn describe_returns_current_page_descriptor() {
    let env = setup("desc");
    let (code, body) = call(&env, "POST", "/navigate", Some(&env.token), Some(json!({ "route": "chat" })));
    assert_eq!(code, 200, "{body}");
    simulate_reply(&env.app_handle(), ok_reply("desc-1", json!({ "name": "chat", "description": "IM", "actions": [] })));
    let (code, body) = call(&env, "GET", "/page/current?requestId=desc-1", Some(&env.token), None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["data"]["schemaVersion"], 1, "必须带 schemaVersion: {body}");
    assert_eq!(body["data"]["page"], "chat", "descriptor 跟随当前路由: {body}");
    assert_eq!(body["data"]["descriptor"]["name"], "chat", "{body}");
}

#[test]
fn action_executes_and_returns_reply() {
    let env = setup("act");
    simulate_reply(&env.app_handle(), ok_reply("act-1", json!({ "id": "m1" })));
    let (code, body) = call(
        &env,
        "POST",
        "/page/action",
        Some(&env.token),
        Some(json!({
            "page": "chat",
            "action": "sendText",
            "args": { "peer": "QmX", "text": "hi" },
            "requestId": "act-1",
        })),
    );
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["data"]["requestId"], "act-1", "回包必须带关联 id: {body}");
    assert_eq!(body["data"]["result"]["id"], "m1", "执行结果透传: {body}");
}

#[test]
fn action_timeout_structured_error() {
    let env = setup("timeout");
    let (code, body) = call(
        &env,
        "POST",
        "/page/action",
        Some(&env.token),
        Some(json!({ "page": "chat", "action": "sendText", "args": {}, "requestId": "act-slow" })),
    );
    assert_eq!(code, 500, "超时必须结构化报错: {body}");
    assert_eq!(body["error"]["code"], "PAGE_TIMEOUT", "{body}");
    assert!(body["error"]["message"].as_str().unwrap_or("").contains("act-slow"), "报错需含关联 id: {body}");
}

#[test]
fn unknown_page_and_missing_fields_rejected() {
    let env = setup("bad");
    let (code, body) = call(
        &env,
        "POST",
        "/page/action",
        Some(&env.token),
        Some(json!({ "page": "nope", "action": "x", "requestId": "r1" })),
    );
    assert_eq!(code, 404, "页名不匹配必须结构化拒绝: {body}");
    assert_eq!(body["error"]["code"], "PAGE_NOT_FOUND", "{body}");
    let (code, body) = call(
        &env,
        "POST",
        "/page/action",
        Some(&env.token),
        Some(json!({ "page": "chat", "action": "x" })),
    );
    assert_eq!(code, 400, "缺 requestId 必须拒绝: {body}");
    assert_eq!(body["error"]["code"], "INVALID_REQUEST", "{body}");
    let (code, body) = call(
        &env,
        "POST",
        "/page/action",
        Some(&env.token),
        Some(json!({ "page": "chat", "requestId": "r2" })),
    );
    assert_eq!(code, 400, "缺 action 必须拒绝: {body}");
    assert_eq!(body["error"]["code"], "INVALID_REQUEST", "{body}");
}

#[test]
fn frontend_rejections_mapped_to_http_status() {
    let env = setup("reject");
    let cases = [
        ("rej-404", "ACTION_NOT_FOUND", 404u16),
        ("rej-confirm", "ACTION_CONFIRM_REQUIRED", 400),
        ("rej-arg", "ARG_TYPE_MISMATCH", 400),
        ("rej-fail", "ACTION_FAILED", 500),
    ];
    for (request_id, code_name, status) in cases {
        simulate_reply(&env.app_handle(), err_reply(request_id, code_name));
        let (code, body) = call(
            &env,
            "POST",
            "/page/action",
            Some(&env.token),
            Some(json!({ "page": "chat", "action": "sendText", "args": {}, "requestId": request_id })),
        );
        assert_eq!(code, status, "{code_name}: {body}");
        assert_eq!(body["error"]["code"], code_name, "{body}");
    }
}

#[test]
fn describe_unregistered_page_maps_to_404() {
    let env = setup("unreg");
    simulate_reply(&env.app_handle(), err_reply("desc-404", "PAGE_NOT_REGISTERED"));
    let (code, body) = call(&env, "GET", "/page/current?requestId=desc-404", Some(&env.token), None);
    assert_eq!(code, 404, "{body}");
    assert_eq!(body["error"]["code"], "PAGE_NOT_REGISTERED", "{body}");
}
