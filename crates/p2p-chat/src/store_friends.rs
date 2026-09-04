//! 好友簿 yrs Doc 承载（Y1 试点）：`chat/friends.json` 改为 yrs 更新日志。
//! 行 1 = 格式头，其余每行 = 一次实际变更的 base64 yrs update（O_APPEND 追加）；
//! yrs update 幂等可交换，双进程并发追加无需文件锁，读取时全量合并（CRDT 语义），
//! remove 走 yrs tombstone。旧 JSON 数组首次载入自动迁移并备份原文件。
//! 好友簿写不再加文件锁（store_lock 退役）；消息 JSONL 不在 CRDT 范围。

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use yrs::updates::decoder::Decode;
use yrs::{Any, Doc, Map, Out, ReadTxn, StateVector, Transact, Update};

use crate::model::ChatFriend;
use crate::store_io::{append_line, atomic_write};

/// 沿用旧文件名：GUI watcher 与运维脚本按 `chat/friends.json` 观测。
pub(crate) const FILE_NAME: &str = "friends.json";

const FORMAT_KEY: &str = "p2p-friends";
const FORMAT_VER: &str = "yrs-v1";
const FRIENDS_MAP: &str = "friends";

pub(crate) struct FriendsBook {
    doc: Doc,
}

impl FriendsBook {
    /// 以磁盘日志为权威态重建（每次好友操作前调用，等价旧实现「锁内重读磁盘」
    /// 的跨进程新鲜度）；文件缺失即建头行。
    pub(crate) fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(raw) => Self::parse(path, raw),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                atomic_write(path, &header_bytes())?;
                Ok(Self { doc: Doc::new() })
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "friends.json 读取失败，按空簿处理"
                );
                Ok(Self { doc: Doc::new() })
            }
        }
    }

    /// 好友全量视图：按 peerId 排序输出（yrs Map 遍历序不稳定，排序保确定性）。
    pub(crate) fn list(&self) -> Vec<ChatFriend> {
        let map = self.doc.get_or_insert_map(FRIENDS_MAP);
        let txn = self.doc.transact();
        let mut out = Vec::new();
        for (key, value) in map.iter(&txn) {
            match decode_friend(&key, &value) {
                Ok(f) => out.push(f),
                Err(e) => tracing::warn!(peer = %key, error = %e, "好友条目解码失败，跳过"),
            }
        }
        out.sort_unstable_by(|a, b| a.peer_id.cmp(&b.peer_id));
        out
    }

    /// 加/改好友（upsert）：内容无变化零追加；有变化以本事务 delta 追加一行。
    pub(crate) fn upsert(&self, path: &Path, friend: ChatFriend) -> io::Result<()> {
        let payload = serde_json::to_string(&friend).map_err(io::Error::other)?;
        let map = self.doc.get_or_insert_map(FRIENDS_MAP);
        let mut txn = self.doc.transact_mut();
        if !map.try_update(&mut txn, friend.peer_id.as_str(), payload) {
            return Ok(());
        }
        let bytes = txn.encode_update_v1();
        drop(txn);
        append_update_line(path, &bytes)
    }

    /// 移除好友：yrs tombstone（并发 add/remove 按 yrs 语义合并）；不在簿 false 且零追加。
    pub(crate) fn remove(&self, path: &Path, peer_id: &str) -> io::Result<bool> {
        let map = self.doc.get_or_insert_map(FRIENDS_MAP);
        let mut txn = self.doc.transact_mut();
        if map.get(&txn, peer_id).is_none() {
            return Ok(false);
        }
        map.remove(&mut txn, peer_id);
        let bytes = txn.encode_update_v1();
        drop(txn);
        append_update_line(path, &bytes)?;
        Ok(true)
    }

    fn parse(path: &Path, raw: String) -> io::Result<Self> {
        if raw.trim_start().starts_with('[') {
            return Self::migrate_legacy(path, &raw);
        }
        let mut lines = raw.lines();
        let header = lines
            .next()
            .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .and_then(|v| v.get(FORMAT_KEY).and_then(|t| t.as_str()).map(str::to_owned));
        if header.as_deref() != Some(FORMAT_VER) {
            tracing::warn!(path = %path.display(), "friends.json 未知格式，原样备份后按空簿重建");
            backup_original(path)?;
            atomic_write(path, &header_bytes())?;
            return Ok(Self { doc: Doc::new() });
        }
        let doc = Doc::new();
        for (i, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            if let Err(e) = apply_log_line(&doc, line) {
                tracing::warn!(path = %path.display(), line = i + 2, error = %e, "更新行损坏，跳过");
            }
        }
        Ok(Self { doc })
    }

    /// 旧 JSON 数组迁移：备份原文件 → 头行 + 全量快照行；损坏旧簿同样备份后按空簿
    /// 处理（数据可追回、路径可观测，语义对齐旧 load_friends 的 warn+空簿）。
    fn migrate_legacy(path: &Path, raw: &str) -> io::Result<Self> {
        let friends: Vec<ChatFriend> = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "friends.json 损坏，原样备份后按空簿处理"
                );
                let backup = backup_original(path)?;
                atomic_write(path, &header_bytes())?;
                tracing::info!(backup = %backup.display(), "损坏旧簿已备份");
                return Ok(Self { doc: Doc::new() });
            }
        };
        let doc = Doc::new();
        let map = doc.get_or_insert_map(FRIENDS_MAP);
        let mut txn = doc.transact_mut();
        for f in &friends {
            let payload = serde_json::to_string(f).map_err(io::Error::other)?;
            map.insert(&mut txn, f.peer_id.as_str(), payload);
        }
        drop(txn);
        let backup = backup_original(path)?;
        let full = {
            let txn = doc.transact();
            format!(
                "{}\n{}\n",
                header_line(),
                update_line(&txn.encode_state_as_update_v1(&StateVector::default()))
            )
        };
        atomic_write(path, full.as_bytes())?;
        tracing::info!(
            path = %path.display(),
            backup = %backup.display(),
            count = friends.len(),
            "旧版 friends.json 迁移为 yrs 更新日志"
        );
        Ok(Self { doc })
    }
}

fn header_line() -> String {
    serde_json::json!({ FORMAT_KEY: FORMAT_VER }).to_string()
}

/// 头行 + 换行（日志为逐行格式，头行必须独立成行）。
fn header_bytes() -> Vec<u8> {
    format!("{}\n", header_line()).into_bytes()
}

fn update_line(bytes: &[u8]) -> String {
    serde_json::json!({ "u": B64.encode(bytes) }).to_string()
}

fn append_update_line(path: &Path, bytes: &[u8]) -> io::Result<()> {
    append_line(path, &update_line(bytes))
}

/// 解析并应用一行更新日志；任何损坏都以 Err 上浮给调用方 warn（不静默吞）。
fn apply_log_line(doc: &Doc, line: &str) -> Result<(), String> {
    let wrap: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let b64 = wrap.get("u").and_then(|v| v.as_str()).ok_or("缺 u 字段")?;
    let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
    let update = Update::decode_v1(&bytes).map_err(|e| e.to_string())?;
    let mut txn = doc.transact_mut();
    txn.apply_update(update).map_err(|e| e.to_string())
}

fn decode_friend(key: &str, value: &Out) -> Result<ChatFriend, String> {
    let Out::Any(Any::String(s)) = value else {
        return Err("条目值非字符串".into());
    };
    let friend: ChatFriend = serde_json::from_str(s.as_ref()).map_err(|e| e.to_string())?;
    if friend.peer_id != key {
        return Err("peerId 与键不一致".into());
    }
    Ok(friend)
}

/// 原文件同目录改名备份（rename 原子），返回备份路径。
fn backup_original(path: &Path) -> io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("{FILE_NAME}.bak-yrs-{nanos}"));
    std::fs::rename(path, &backup)?;
    Ok(backup)
}
