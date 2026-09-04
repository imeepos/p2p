//! 控制通道客户端：与运行中守护进程的 daemon.sock 做 JSON 行协议往返。
//! 请求一行 JSON，响应一行 {"ok":bool,"data"|"error"}；连接失败即节点未运行。

use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::CliError;
use crate::paths::Paths;

/// 与守护进程完成一次请求往返；快操作 5s 超时，ok=false 转为运行失败错误。
pub async fn call(paths: &Paths, request: Value) -> Result<Value, CliError> {
    unwrap_response(raw_call(paths, request, Some(Duration::from_secs(5))).await?)
}

/// 慢操作（dial/ping 走真实网络降级链）用放宽超时，语义同 [call]。
pub async fn call_slow(paths: &Paths, request: Value, timeout: Duration) -> Result<Value, CliError> {
    unwrap_response(raw_call(paths, request, Some(timeout)).await?)
}

/// 拆响应包装：ok=true 取 data；ok=false 取 error；形态异常显式报错。
fn unwrap_response(response: Value) -> Result<Value, CliError> {
    match response {
        Value::Object(map) if map.get("ok") == Some(&Value::Bool(true)) => {
            Ok(map.get("data").cloned().unwrap_or(Value::Null))
        }
        Value::Object(map) => {
            let msg = map
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("守护进程返回未知错误")
                .to_string();
            Err(CliError::Runtime(msg))
        }
        other => Err(CliError::Runtime(format!("守护进程响应格式非法: {other}"))),
    }
}

/// 连接 + 单请求往返；timeout=None 表示不限时（由调用方约束）。
async fn raw_call(paths: &Paths, request: Value, timeout: Option<Duration>) -> Result<Value, CliError> {
    let fut = async {
        let stream = UnixStream::connect(paths.sock())
            .await
            .map_err(|e| CliError::Runtime(format!("连接节点守护进程失败: {e}")))?;
        exchange(stream, request).await
    };
    match timeout {
        Some(t) => match tokio::time::timeout(t, fut).await {
            Ok(result) => result,
            Err(_) => Err(CliError::Runtime(format!("守护进程响应超时（{t:?}）"))),
        },
        None => fut.await,
    }
}

async fn exchange(stream: UnixStream, request: Value) -> Result<Value, CliError> {
    let mut line = request.to_string();
    line.push('\n');
    let (reader, mut writer) = stream.into_split();
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| CliError::Runtime(format!("发送控制请求失败: {e}")))?;
    let mut buf = String::new();
    BufReader::new(reader)
        .read_line(&mut buf)
        .await
        .map_err(|e| CliError::Runtime(format!("读取控制响应失败: {e}")))?;
    if buf.trim().is_empty() {
        return Err(CliError::Runtime("守护进程已断开连接".into()));
    }
    serde_json::from_str(buf.trim())
        .map_err(|e| CliError::Runtime(format!("控制响应解析失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::unwrap_response;
    use serde_json::json;

    #[test]
    fn ok_response_unwraps_data() {
        let data = unwrap_response(json!({ "ok": true, "data": { "running": true } })).unwrap();
        assert_eq!(data["running"], json!(true));
    }

    #[test]
    fn error_response_becomes_message() {
        let err = unwrap_response(json!({ "ok": false, "error": "节点未运行" })).unwrap_err();
        assert!(err.to_string().contains("节点未运行"));
    }

    #[test]
    fn malformed_response_rejected() {
        assert!(unwrap_response(json!("junk")).is_err());
    }
}
