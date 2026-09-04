//! stdout JSON 行输出（D 的 CLI 可读面）：就绪信息、发现清单、状态迁移。
//! 每行一个 JSON 对象：{"kind":<事件类型>,...载荷}；序列化异常落 stderr，不静默。

use serde::Serialize;
use serde_json::{Map, Value};

/// 打一行 {"kind":<kind>,...payload}。载荷必须是 JSON object。
pub fn event(kind: &str, payload: &impl Serialize) {
    let mut obj = Map::new();
    obj.insert("kind".to_string(), Value::from(kind));
    match serde_json::to_value(payload) {
        Ok(Value::Object(rest)) => obj.extend(rest),
        Ok(other) => {
            eprintln!("acp-console: stdout payload not an object: kind={kind} got={other}");
        }
        Err(err) => {
            eprintln!("acp-console: stdout payload serialize failed: kind={kind} err={err}");
        }
    }
    println!("{}", Value::Object(obj));
}
