//! 群聊本地存储（design §4）：groups.json/goutbox/群历史；复用 store_io 与 store_lock。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::group::{GroupKick, GroupLeave, GroupRoster};
use crate::model::{sanitize_name, ChatError, ChatKind, ChatMediaMeta, ChatStatus};
use crate::store_io::{append_line, atomic_write, dedup_last, load_jsonl, rewrite_jsonl};
use crate::store_lock::FileLock;

/// 群名上限（trim 后字符数，design §1）。
pub(crate) const MAX_GROUP_NAME_CHARS: usize = 64;
/// 群成员上限（含 owner，design §1）。
pub(crate) const MAX_GROUP_MEMBERS: usize = 32;
const GROUPS_LOCK_TIMEOUT /* 同 friends 纪律，超时显式报错拒绝静默覆盖 */: Duration = Duration::from_secs(10);

/// 群状态（design §4）：退群/被踢/解散不删数据，state 置位，历史保留。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupState {
    Active,
    Left,
    Kicked,
    Disbanded,
}

/// groups.json 条目（契约 GroupJson；members 含 owner）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub group_id: String,
    pub name: String,
    pub owner: String,
    pub members: Vec<String>,
    pub rev: u64,
    pub state: GroupState,
    pub ts_ms: i64,
}

impl GroupInfo {
    /// 首见 roster 落地：state 直接 active（收到 roster 即在群，支持重邀回归）。
    pub(crate) fn from_roster(r: &GroupRoster) -> Self {
        Self {
            group_id: r.group_id.clone(),
            name: r.name.clone(),
            owner: r.owner.clone(),
            members: r.members.clone(),
            rev: r.rev,
            state: GroupState::Active,
            ts_ms: r.ts_ms,
        }
    }
}

/// 群历史条目（本地形状，JSONL 一行一条；status/path/acks 为本地字段不跨网）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMessage {
    pub id: String,
    pub group_id: String,
    /// 作者 PeerId；本端消息判定 sender_id == 本机 PeerId。
    pub sender_id: String,
    pub kind: ChatKind,
    pub ts_ms: i64,
    pub text: Option<String>,
    pub media: Option<ChatMediaMeta>,
    pub status: ChatStatus,
    /// 已 ACK 成员（仅本端发出的消息维护，收到的恒空）。
    #[serde(default)]
    pub acks: Vec<String>,
    pub reply_to: Option<String>,
}

/// goutbox 载荷（design §3/§5）：消息事务帧 + roster/kick/leave 离线补投帧。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum GoutboxFrame {
    Msg { msg: Box<GroupMessage> },
    Roster { roster: Box<GroupRoster> },
    Kick { kick: Box<GroupKick> },
    Leave { leave: Box<GroupLeave> },
}

/// goutbox/<to>.jsonl 行；status = 条目投递状态（pending/failed，复用 ChatStatus）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoutboxEntry {
    pub id: String,
    pub to: String,
    pub status: ChatStatus,
    #[serde(flatten)]
    pub frame: GoutboxFrame,
}

/// groups.json 读取：损坏/缺失回退空簿并 warn（同 friends 纪律）。
fn load_groups(path: &Path) -> Vec<GroupInfo> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "groups.json 损坏，按空簿处理");
                Vec::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "groups.json 读取失败，按空簿处理");
            Vec::new()
        }
    }
}

/// 群存储（同步 std::fs：文件小、频率低，与 1:1 store 同风格）。
pub(crate) struct GroupStore {
    groups_path: PathBuf,
    goutbox_dir: PathBuf,
    history_dir: PathBuf,
    media_dir: PathBuf,
}

impl GroupStore {
    pub(crate) fn new(chat_dir: PathBuf) -> std::io::Result<Self> {
        let goutbox_dir = chat_dir.join("goutbox");
        let history_dir = chat_dir.join("groups");
        let media_dir = chat_dir.join("media");
        fs::create_dir_all(&goutbox_dir)?;
        fs::create_dir_all(&history_dir)?;
        fs::create_dir_all(&media_dir)?;
        Ok(Self {
            groups_path: chat_dir.join("groups.json"),
            goutbox_dir,
            history_dir,
            media_dir,
        })
    }

    pub(crate) fn groups_list(&self) -> Vec<GroupInfo> {
        load_groups(&self.groups_path)
    }

    pub(crate) fn group(&self, group_id: &str) -> Option<GroupInfo> {
        self.groups_list()
            .into_iter()
            .find(|g| g.group_id == group_id)
    }

    /// 原子写 groups.json：跨进程锁内「重读磁盘 → 合并 → 写」（friends 同纪律）。
    pub(crate) fn save_group(&self, group: GroupInfo) -> std::io::Result<()> {
        let lock_path = PathBuf::from(format!("{}.lock", self.groups_path.display()));
        let _lock = FileLock::acquire(lock_path, GROUPS_LOCK_TIMEOUT)?;
        let mut list = load_groups(&self.groups_path);
        match list.iter_mut().find(|g| g.group_id == group.group_id) {
            Some(slot) => *slot = group,
            None => list.push(group),
        }
        let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
        atomic_write(&self.groups_path, &bytes)
    }

    pub(crate) fn goutbox_for(&self, to: &str) -> Vec<GoutboxEntry> {
        dedup_last(load_jsonl(&self.goutbox_path(to)), |e| &e.id)
    }

    pub(crate) fn append_goutbox(&self, entry: &GoutboxEntry) -> std::io::Result<()> {
        let line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
        append_line(&self.goutbox_path(&entry.to), &line)
    }

    pub(crate) fn remove_goutbox(&self, to: &str, entry_id: &str) -> std::io::Result<()> {
        rewrite_jsonl::<GoutboxEntry>(&self.goutbox_path(to), |e| e.id != entry_id)
    }

    pub(crate) fn set_goutbox_status(
        &self,
        to: &str,
        entry_id: &str,
        status: ChatStatus,
    ) -> std::io::Result<()> {
        rewrite_jsonl::<GoutboxEntry>(&self.goutbox_path(to), |e| {
            if e.id == entry_id {
                e.status = status;
            }
            true
        })
    }

    /// 群历史（懒读 + (groupId,id) 去重：同 id 多行以最后一行为准）。
    pub(crate) fn history(&self, group_id: &str) -> Vec<GroupMessage> {
        dedup_last(load_jsonl(&self.history_path(group_id)), |m| &m.id)
    }

    pub(crate) fn append_message(&self, msg: &GroupMessage) -> std::io::Result<()> {
        let line = serde_json::to_string(msg).map_err(std::io::Error::other)?;
        append_line(&self.history_path(&msg.group_id), &line)
    }

    /// 命中 id 的历史行原位修补（状态推进 / acks 追加）。
    pub(crate) fn patch_message(
        &self,
        group_id: &str,
        id: &str,
        f: impl Fn(&mut GroupMessage),
    ) -> std::io::Result<()> {
        rewrite_jsonl::<GroupMessage>(&self.history_path(group_id), move |m| {
            if m.id == id {
                f(m);
            }
            true
        })
    }

    pub(crate) fn media_group_dir(&self, group_id: &str) -> std::io::Result<PathBuf> {
        let dir = self.media_dir.join(group_id);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// 发端附件落盘：tmp + rename（media/<groupId>/，UUID ≠ base58 不相交）。
    pub(crate) fn save_media(
        &self,
        group_id: &str,
        msg_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> std::io::Result<PathBuf> {
        let dir = self.media_group_dir(group_id)?;
        let final_path = dir.join(format!("{msg_id}_{}", sanitize_name(name)));
        let tmp = dir.join(format!(".tmp-{msg_id}-{}", std::process::id()));
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &final_path)?;
        Ok(final_path)
    }

    fn goutbox_path(&self, to: &str) -> PathBuf {
        self.goutbox_dir.join(format!("{to}.jsonl"))
    }

    fn history_path(&self, group_id: &str) -> PathBuf {
        self.history_dir.join(format!("{group_id}.jsonl"))
    }
}

/// 群名校验：trim 后 1..=64 字符（design §1 硬边界）。
pub(crate) fn validate_group_name(raw: &str) -> Result<String, ChatError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(ChatError::InvalidGroup("群名为空".into()));
    }
    if name.chars().count() > MAX_GROUP_NAME_CHARS {
        return Err(ChatError::InvalidGroup(format!(
            "群名超过 {MAX_GROUP_NAME_CHARS} 字符上限"
        )));
    }
    Ok(name.to_string())
}
