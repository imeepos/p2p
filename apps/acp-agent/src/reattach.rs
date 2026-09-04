//! 续连缓存与 wire 行谓词（设计 §5）：客户端断流窗口内 session/update 逐条入
//! 每会话环形缓存，上限 8 MiB，超限丢最旧并留日志；重连补放后清空。
//! 补放前由桥在透传层前置一行桥约定通知（README 契约段）。

use std::collections::{BTreeMap, VecDeque};

use acp_common::consts::SESSION_UPDATE_CACHE_LIMIT;
use serde_json::{json, Value};

/// 桥约定通知：补放条数宣告（通知无 id，重放协议合法）。
pub const REPLAY_ANNOUNCE_METHOD: &str = "dsh/bridge/reattach";

#[derive(Default)]
struct SessionQueue {
    lines: VecDeque<Vec<u8>>,
    bytes: usize,
}

pub struct UpdateCache {
    limit: usize,
    sessions: BTreeMap<String, SessionQueue>,
}

impl Default for UpdateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateCache {
    pub fn new() -> Self {
        Self::with_limit(SESSION_UPDATE_CACHE_LIMIT)
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            sessions: BTreeMap::new(),
        }
    }

    /// 入队一行（不含换行）；返回因超限被丢弃的最旧行数。
    pub fn push(&mut self, session: &str, line: Vec<u8>) -> usize {
        let bytes = line.len();
        let queue = self.sessions.entry(session.to_owned()).or_default();
        queue.lines.push_back(line);
        queue.bytes += bytes;
        let mut dropped = 0;
        while queue.bytes > self.limit {
            let Some(oldest) = queue.lines.pop_front() else {
                break;
            };
            queue.bytes -= oldest.len();
            dropped += 1;
        }
        dropped
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.values().all(|q| q.lines.is_empty())
    }

    /// 补放：会话名序 + 行序产出（不含换行）并清空。
    pub fn drain(&mut self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for queue in self.sessions.values_mut() {
            out.extend(queue.lines.drain(..));
            queue.bytes = 0;
        }
        out
    }
}

/// session/update 行的会话键（params.sessionId）。
pub fn update_session_key(root: &Value) -> Option<&str> {
    root.get("params")?.get("sessionId")?.as_str()
}

pub fn is_session_update(root: &Value) -> bool {
    root.get("method").and_then(Value::as_str) == Some("session/update")
}

pub fn is_method(root: &Value, name: &str) -> bool {
    root.get("method").and_then(Value::as_str) == Some(name)
}

/// JSON-RPC 响应行：有 id 且无 method。
pub fn is_response(root: &Value) -> bool {
    root.get("method").is_none() && root.get("id").is_some_and(|id| !id.is_null())
}

/// 补放宣告行（桥约定，GUI 据此显示"已续连，补放 N 条错过的更新"）。
pub fn replay_announcement(count: usize) -> Vec<u8> {
    json!({
        "jsonrpc": "2.0",
        "method": REPLAY_ANNOUNCE_METHOD,
        "params": { "replayed": count },
    })
    .to_string()
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_wraps_dropping_oldest() {
        let mut cache = UpdateCache::with_limit(12);
        assert_eq!(cache.push("s1", b"0123456789".to_vec()), 0);
        assert_eq!(cache.push("s1", b"abc".to_vec()), 1);
        assert_eq!(cache.push("s1", b"def".to_vec()), 0);
        let drained = cache.drain();
        assert_eq!(drained, vec![b"abc".to_vec(), b"def".to_vec()]);
        assert!(cache.is_empty());
    }

    #[test]
    fn eight_mib_limit_wraps_large_lines() {
        let mut cache = UpdateCache::new();
        let big = vec![b'x'; SESSION_UPDATE_CACHE_LIMIT / 2];
        assert_eq!(cache.push("s", big.clone()), 0);
        assert_eq!(cache.push("s", big.clone()), 0);
        assert_eq!(cache.push("s", big), 1);
        let drained = cache.drain();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn drain_orders_sessions_by_name_then_insertion() {
        let mut cache = UpdateCache::with_limit(1024);
        cache.push("s2", b"b1".to_vec());
        cache.push("s1", b"a1".to_vec());
        cache.push("s2", b"b2".to_vec());
        let drained = cache.drain();
        assert_eq!(
            drained,
            vec![b"a1".to_vec(), b"b1".to_vec(), b"b2".to_vec()]
        );
    }

    #[test]
    fn wire_predicates() {
        assert!(is_session_update(
            &json!({"method": "session/update", "params": {"sessionId": "s"}})
        ));
        assert_eq!(
            update_session_key(&json!({"params": {"sessionId": "s"}})),
            Some("s")
        );
        assert!(is_initialize_like(
            &json!({"method": "initialize", "id": 1})
        ));
        assert!(is_response(
            &json!({"jsonrpc": "2.0", "id": 3, "result": {}})
        ));
        assert!(!is_response(
            &json!({"jsonrpc": "2.0", "id": 3, "method": "ping"})
        ));
        assert!(!is_response(&json!({"jsonrpc": "2.0", "id": null})));
        let announce = replay_announcement(4);
        let root: Value = serde_json::from_slice(&announce).expect("json");
        assert_eq!(root["params"]["replayed"], 4);
    }

    fn is_initialize_like(root: &Value) -> bool {
        is_method(root, "initialize")
    }
}
