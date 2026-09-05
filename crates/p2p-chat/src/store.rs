//! 本地存储（design §4，Y1 起）：好友簿由 yrs Doc 承载（store_friends.rs，无锁
//! CRDT 合并）；outbox/messages 追加式 JSONL；损坏行读取时跳过并 warn；状态变更
//! 重写同文件时原样保留未知行（含损坏行）。文件级 helper 见 store_io.rs；
//! 非测试代码禁 unwrap/expect/panic。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use std::time::Duration;

use crate::invite::FriendInvite;
use crate::model::{sanitize_name, ChatEnvelope, ChatFriend, ChatStatus};
use crate::store_friends::FriendsBook;
use crate::store_io::{
    append_line, dedup_last_by_id, load_invites, load_jsonl, rewrite_jsonl_patch_status,
    rewrite_jsonl_retain,
};

pub(crate) const FRIENDS_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn poisoned() -> std::io::Error {
    std::io::Error::other("store 内部锁中毒")
}

pub(crate) struct Store {
    pub(crate) friends_path: PathBuf,
    pub(crate) invites_path: PathBuf,
    pub(crate) advertised_path: PathBuf,
    outbox_dir: PathBuf,
    messages_dir: PathBuf,
    media_dir: PathBuf,
    pub(crate) lock_timeout: Duration,
    pub(crate) state: Mutex<State>,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) invites: Vec<FriendInvite>,
    outbox: HashMap<String, Vec<ChatEnvelope>>,
    messages: HashMap<String, Vec<ChatEnvelope>>,
}

impl Store {
    pub(crate) fn new(chat_dir: PathBuf) -> std::io::Result<Self> {
        let friends_path = chat_dir.join("friends.json");
        let invites_path = chat_dir.join("invites.json");
        let advertised_path = chat_dir.join(crate::advertised::ADVERTISED_FILE);
        let outbox_dir = chat_dir.join("outbox");
        let messages_dir = chat_dir.join("messages");
        let media_dir = chat_dir.join("media");
        fs::create_dir_all(&outbox_dir)?;
        fs::create_dir_all(&messages_dir)?;
        fs::create_dir_all(&media_dir)?;
        FriendsBook::load(&friends_path)?;
        let invites = load_invites(&invites_path);
        let mut outbox = HashMap::new();
        if let Ok(entries) = fs::read_dir(&outbox_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let peer = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(p) => p.to_string(),
                    None => continue,
                };
                let envs = dedup_last_by_id(load_jsonl::<ChatEnvelope>(&path));
                if !envs.is_empty() {
                    outbox.insert(peer, envs);
                }
            }
        }
        Ok(Self {
            friends_path,
            invites_path,
            advertised_path,
            outbox_dir,
            messages_dir,
            media_dir,
            lock_timeout: FRIENDS_LOCK_TIMEOUT,
            state: Mutex::new(State {
                invites,
                outbox,
                messages: HashMap::new(),
            }),
        })
    }

    /// 好友全量视图：每次直读磁盘 yrs 日志合并（跨进程写入即刻可见）。
    pub(crate) fn friends_list(&self) -> Result<Vec<ChatFriend>, std::io::Error> {
        Ok(FriendsBook::load(&self.friends_path)?.list())
    }

    /// 加/改好友： yrs 合并语义（update 追加，无文件锁），并发写只增不丢。
    pub(crate) fn upsert_friend(&self, friend: ChatFriend) -> Result<(), std::io::Error> {
        FriendsBook::load(&self.friends_path)?.upsert(&self.friends_path, friend)
    }

    /// 移除好友：yrs tombstone；不在簿返回 false 且零追加。
    pub(crate) fn remove_friend(&self, peer_id: &str) -> Result<bool, std::io::Error> {
        FriendsBook::load(&self.friends_path)?.remove(&self.friends_path, peer_id)
    }

    pub(crate) fn outbox_for(&self, peer: &str) -> Vec<ChatEnvelope> {
        match self.state.lock() {
            Ok(state) => state.outbox.get(peer).cloned().unwrap_or_default(),
            Err(_) => {
                tracing::warn!(peer = %peer, "outbox 读取时锁中毒，按空处理");
                Vec::new()
            }
        }
    }

    pub(crate) fn append_outbox(&self, env: &ChatEnvelope) -> Result<(), std::io::Error> {
        let mut state = self.state.lock().map_err(|_| poisoned())?;
        let line = serde_json::to_string(env).map_err(std::io::Error::other)?;
        append_line(&self.outbox_path(&env.peer), &line)?;
        state
            .outbox
            .entry(env.peer.clone())
            .or_default()
            .push(env.clone());
        Ok(())
    }

    pub(crate) fn remove_outbox(&self, peer: &str, id: &str) -> Result<(), std::io::Error> {
        {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            if let Some(entries) = state.outbox.get_mut(peer) {
                entries.retain(|e| e.id != id);
                if entries.is_empty() {
                    state.outbox.remove(peer);
                }
            }
        }
        rewrite_jsonl_retain(&self.outbox_path(peer), |e| e.id != id)
    }

    pub(crate) fn set_outbox_status(
        &self,
        peer: &str,
        id: &str,
        status: ChatStatus,
    ) -> Result<bool, std::io::Error> {
        {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            if let Some(entries) = state.outbox.get_mut(peer) {
                for e in entries.iter_mut() {
                    if e.id == id {
                        e.status = status;
                    }
                }
            }
        }
        rewrite_jsonl_patch_status(&self.outbox_path(peer), id, status)
    }

    pub(crate) fn messages_for(&self, peer: &str) -> Result<Vec<ChatEnvelope>, std::io::Error> {
        let mut state = self.state.lock().map_err(|_| poisoned())?;
        if !state.messages.contains_key(peer) {
            let envs = dedup_last_by_id(load_jsonl::<ChatEnvelope>(&self.messages_path(peer)));
            state.messages.insert(peer.to_string(), envs);
        }
        Ok(state.messages.get(peer).cloned().unwrap_or_default())
    }

    /// 以磁盘真值判定（跨进程写入后内存视图会过期）；同时把归并后的磁盘视图刷回缓存。
    pub(crate) fn has_message(&self, peer: &str, id: &str) -> bool {
        let fresh = dedup_last_by_id(load_jsonl::<ChatEnvelope>(&self.messages_path(peer)));
        let hit = fresh.iter().any(|m| m.id == id);
        if let Ok(mut state) = self.state.lock() {
            state.messages.insert(peer.to_string(), fresh);
        }
        hit
    }

    pub(crate) fn append_message(&self, env: &ChatEnvelope) -> Result<(), std::io::Error> {
        let mut state = self.state.lock().map_err(|_| poisoned())?;
        let line = serde_json::to_string(env).map_err(std::io::Error::other)?;
        append_line(&self.messages_path(&env.peer), &line)?;
        state
            .messages
            .entry(env.peer.clone())
            .or_default()
            .push(env.clone());
        Ok(())
    }

    /// 更新状态；返回磁盘是否有命中行（未命中=该消息不在本进程磁盘视图，由调用方决定追加）。
    pub(crate) fn update_message_status(
        &self,
        peer: &str,
        id: &str,
        status: ChatStatus,
    ) -> Result<bool, std::io::Error> {
        {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            if let Some(msgs) = state.messages.get_mut(peer) {
                for m in msgs.iter_mut() {
                    if m.id == id {
                        m.status = status;
                    }
                }
            }
        }
        rewrite_jsonl_patch_status(&self.messages_path(peer), id, status)
    }

    pub(crate) fn media_peer_dir(&self, peer: &str) -> Result<PathBuf, std::io::Error> {
        let dir = self.media_dir.join(peer);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// 发端附件落盘：tmp + rename（文件名 = msgId + sanitize 后的原始名）。
    pub(crate) fn save_media(
        &self,
        peer: &str,
        msg_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, std::io::Error> {
        let dir = self.media_peer_dir(peer)?;
        let final_path = dir.join(format!("{msg_id}_{}", sanitize_name(name)));
        let tmp = dir.join(format!(".tmp-{msg_id}-{}", std::process::id()));
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &final_path)?;
        Ok(final_path)
    }

    pub(crate) fn invites_lock_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.lock", self.invites_path.display()))
    }
    fn outbox_path(&self, peer: &str) -> PathBuf {
        self.outbox_dir.join(format!("{peer}.jsonl"))
    }

    fn messages_path(&self, peer: &str) -> PathBuf {
        self.messages_dir.join(format!("{peer}.jsonl"))
    }
}
