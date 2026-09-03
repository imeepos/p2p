//! JSONL / friends.json / 原子写的通用文件 helper（store.rs 拆分，行数红线）。

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;

use crate::model::{ChatEnvelope, ChatFriend, ChatStatus};

/// friends.json 数组读取：损坏或缺失回退空簿并留 warn（不静默）。
pub(crate) fn load_friends(path: &Path) -> Vec<ChatFriend> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<Vec<ChatFriend>>(&content) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "friends.json 损坏，按空簿处理");
                Vec::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "friends.json 读取失败，按空簿处理");
            Vec::new()
        }
    }
}

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

/// 重写 JSONL：id 命中行更新 status，其余行（含损坏行）原样保留。
pub(crate) fn rewrite_jsonl_patch_status(
    path: &Path,
    id: &str,
    status: ChatStatus,
) -> Result<(), std::io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut out = String::new();
    for line in content.lines() {
        match serde_json::from_str::<ChatEnvelope>(line) {
            Ok(mut env) if env.id == id => {
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
    atomic_write(path, out.as_bytes())
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
