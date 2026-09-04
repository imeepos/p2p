//! JSONL / 原子写的通用文件 helper（store.rs 拆分，行数红线）。
//! 好友簿 yrs 日志的文件操作在 store_friends.rs，不经本模块。

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;

use crate::model::{ChatEnvelope, ChatStatus};

/// 读 JSONL：损坏行跳过并 warn（缺失文件 = 空）。
pub(crate) fn load_jsonl<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "JSONL 读取失败，按空处理");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        match serde_json::from_str::<T>(line) {
            Ok(v) => out.push(v),
            Err(e) => {
                tracing::warn!(path = %path.display(), line = i + 1, error = %e, "损坏行跳过")
            }
        }
    }
    out
}

/// 装载归并：同 id 多行（跨进程写交错产物）以最后一行为准，按首现位置排序。
pub(crate) fn dedup_last_by_id(envelopes: Vec<ChatEnvelope>) -> Vec<ChatEnvelope> {
    let mut order: Vec<String> = Vec::new();
    let mut last: std::collections::HashMap<String, ChatEnvelope> =
        std::collections::HashMap::new();
    for env in envelopes {
        if !last.contains_key(&env.id) {
            order.push(env.id.clone());
        }
        last.insert(env.id.clone(), env);
    }
    order
        .into_iter()
        .filter_map(|id| last.remove(&id))
        .collect()
}

/// 追加一行 JSONL（文件不存在即创建）。
pub(crate) fn append_line(path: &Path, line: &str) -> Result<(), std::io::Error> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(line.as_bytes())?;
    f.write_all(b"\n")?;
    f.flush()
}

/// 重写 JSONL：不满足 keep 的已解析消息行删除，其余行（含损坏行）原样保留。
pub(crate) fn rewrite_jsonl_retain(
    path: &Path,
    keep: impl Fn(&ChatEnvelope) -> bool,
) -> Result<(), std::io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut out = String::new();
    for line in content.lines() {
        match serde_json::from_str::<ChatEnvelope>(line) {
            Ok(env) if !keep(&env) => {}
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    atomic_write(path, out.as_bytes())
}

/// 重写 JSONL：id 命中行更新 status，其余行（含损坏行）原样保留；返回是否有命中。
pub(crate) fn rewrite_jsonl_patch_status(
    path: &Path,
    id: &str,
    status: ChatStatus,
) -> Result<bool, std::io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let mut out = String::new();
    let mut hit = false;
    for line in content.lines() {
        match serde_json::from_str::<ChatEnvelope>(line) {
            Ok(mut env) if env.id == id => {
                hit = true;
                env.status = status;
                let bytes = serde_json::to_string(&env).map_err(std::io::Error::other)?;
                out.push_str(&bytes);
                out.push('\n');
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    atomic_write(path, out.as_bytes())?;
    Ok(hit)
}

/// 原子写：同目录 tmp + rename（进程内唯一后缀防并发碰撞）。
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("路径无父目录"))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".tmp-{}-{nanos}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with_status(id: &str, status: ChatStatus) -> ChatEnvelope {
        ChatEnvelope {
            id: id.to_string(),
            peer: "p".into(),
            sender: crate::model::Sender::Me,
            kind: crate::model::ChatKind::Text,
            ts_ms: 1,
            text: Some("t".into()),
            media: None,
            status,
            reply_to: None,
        }
    }

    #[test]
    fn dedup_keeps_last_line_per_id_in_first_seen_order() {
        let envs = vec![
            env_with_status("a", ChatStatus::Pending),
            env_with_status("b", ChatStatus::Sent),
            env_with_status("a", ChatStatus::Delivered),
        ];
        let merged = dedup_last_by_id(envs);
        let ids: Vec<&str> = merged.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
        assert_eq!(merged[0].status, ChatStatus::Delivered);
        assert_eq!(merged[1].status, ChatStatus::Sent);
    }
}
