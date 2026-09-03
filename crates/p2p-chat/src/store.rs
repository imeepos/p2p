//! 本地存储（design §4）：friends.json 原子写；outbox/messages 追加式 JSONL；
//! 损坏行读取时跳过并 warn；状态变更重写同文件时原样保留未知行（含损坏行）。
//! 文件级 helper（JSONL/原子写）见 store_io.rs；非测试代码禁 unwrap/expect/panic。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::model::{sanitize_name, ChatEnvelope, ChatFriend, ChatStatus};
use crate::store_io::{
    append_line, atomic_write, load_friends, load_jsonl, rewrite_jsonl_patch_status,
    rewrite_jsonl_retain,
};

fn poisoned() -> std::io::Error {
    std::io::Error::other("store 内部锁中毒")
}

pub(crate) struct Store {
    friends_path: PathBuf,
    outbox_dir: PathBuf,
    messages_dir: PathBuf,
    media_dir: PathBuf,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    friends: Vec<ChatFriend>,
    outbox: HashMap<String, Vec<ChatEnvelope>>,
    messages: HashMap<String, Vec<ChatEnvelope>>,
}

impl Store {
    pub(crate) fn new(chat_dir: PathBuf) -> std::io::Result<Self> {
        let friends_path = chat_dir.join("friends.json");
        let outbox_dir = chat_dir.join("outbox");
        let messages_dir = chat_dir.join("messages");
        let media_dir = chat_dir.join("media");
        fs::create_dir_all(&outbox_dir)?;
        fs::create_dir_all(&messages_dir)?;
        fs::create_dir_all(&media_dir)?;
        let friends = load_friends(&friends_path);
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
                let envs = load_jsonl::<ChatEnvelope>(&path);
                if !envs.is_empty() {
                    outbox.insert(peer, envs);
                }
            }
        }
        Ok(Self {
            friends_path,
            outbox_dir,
            messages_dir,
            media_dir,
            state: Mutex::new(State {
                friends,
                outbox,
                messages: HashMap::new(),
            }),
        })
    }

    pub(crate) fn friends_list(&self) -> Result<Vec<ChatFriend>, std::io::Error> {
        let state = self.state.lock().map_err(|_| poisoned())?;
        Ok(state.friends.clone())
    }

    pub(crate) fn upsert_friend(&self, friend: ChatFriend) -> Result<(), std::io::Error> {
        let list = {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            if let Some(existing) = state
                .friends
                .iter_mut()
                .find(|f| f.peer_id == friend.peer_id)
            {
                *existing = friend;
            } else {
                state.friends.push(friend);
            }
            state.friends.clone()
        };
        let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
        atomic_write(&self.friends_path, &bytes)
    }

    pub(crate) fn remove_friend(&self, peer_id: &str) -> Result<bool, std::io::Error> {
        let (removed, list) = {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            let before = state.friends.len();
            state.friends.retain(|f| f.peer_id != peer_id);
            (state.friends.len() != before, state.friends.clone())
        };
        if removed {
            let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
            atomic_write(&self.friends_path, &bytes)?;
        }
        Ok(removed)
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
    ) -> Result<(), std::io::Error> {
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
            let envs = load_jsonl::<ChatEnvelope>(&self.messages_path(peer));
            state.messages.insert(peer.to_string(), envs);
        }
        Ok(state.messages.get(peer).cloned().unwrap_or_default())
    }

    pub(crate) fn has_message(&self, peer: &str, id: &str) -> bool {
        self.messages_for(peer)
            .map(|msgs| msgs.iter().any(|m| m.id == id))
            .unwrap_or(false)
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

    pub(crate) fn update_message_status(
        &self,
        peer: &str,
        id: &str,
        status: ChatStatus,
    ) -> Result<(), std::io::Error> {
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

    fn outbox_path(&self, peer: &str) -> PathBuf {
        self.outbox_dir.join(format!("{peer}.jsonl"))
    }

    fn messages_path(&self, peer: &str) -> PathBuf {
        self.messages_dir.join(format!("{peer}.jsonl"))
    }
}
