//! 分享台账数据模型与 token 工具（docs/design/acp-share-design.md §2/§3）：
//! ShareEntry 只存 token sha256，原文永不落盘/进日志/进审计；文件信封 version=1，
//! 存取沿策略表先例（tmp+rename 原子写，缺失由调用方定夺，损坏/版本不符显式报错）。
//! 纯库：路径与时钟由调用方注入，不做 IO 编排。

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::ErrorCode;
use crate::policy::{AskRoute, Scope};

/// 台账文件信封版本。
pub const SHARES_FILE_VERSION: u32 = 1;
/// 激活次数默认值（设计 §3：一次性 = 激活次数，默认 1）。
pub const DEFAULT_MAX_ACTIVATIONS: u32 = 1;
/// 兑换写入策略表的 fingerprint 前缀（设计 §4）；撤销级联按此前缀识别 share 来源。
pub const SHARE_FINGERPRINT_PREFIX: &str = "share:";
/// 链接 scheme（设计 §2 冻结契约）。
pub const SHARE_LINK_SCHEME: &str = "dsh-acp-share";

/// 创建入参（admin POST /shares 与 p2pctl acp share create 共用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareSpec {
    pub scope: Scope,
    pub allow_mcp: Vec<String>,
    pub ask_route: AskRoute,
    pub max_activations: u32,
    pub note: String,
    pub ttl_secs: u64,
    /// 定向工作区 id（scope=workspace 时生效；None = 默认工作区）。
    pub workspace: Option<String>,
}

/// 单条分享台账（设计 §3）。token 原文只存在于创建响应与链接里一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareEntry {
    pub share_id: Uuid,
    pub token_sha256: String,
    pub scope: Scope,
    #[serde(default)]
    pub allow_mcp: Vec<String>,
    pub ask_route: AskRoute,
    #[serde(default = "default_max_activations")]
    pub max_activations: u32,
    #[serde(default)]
    pub activations: u32,
    pub expires_at_unix: u64,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub note: String,
    pub created_at: String,
    #[serde(default)]
    pub bound_peer: Option<String>,
    /// 定向工作区 id（追加字段，台账 v1 旧文件缺省 = None = 默认工作区）。
    #[serde(default)]
    pub workspace: Option<String>,
}

fn default_max_activations() -> u32 {
    DEFAULT_MAX_ACTIVATIONS
}

impl ShareEntry {
    /// 创建条目：token 以原文传入仅供算哈希，本函数不持有、不返回原文。
    pub fn new(spec: ShareSpec, token: &str, now_secs: u64, share_id: Uuid) -> Self {
        Self {
            share_id,
            token_sha256: token_sha256(token),
            scope: spec.scope,
            allow_mcp: spec.allow_mcp,
            ask_route: spec.ask_route,
            max_activations: spec.max_activations.max(1),
            activations: 0,
            expires_at_unix: now_secs.saturating_add(spec.ttl_secs),
            revoked: false,
            note: spec.note,
            created_at: rfc3339_from_unix(now_secs),
            bound_peer: None,
            workspace: spec.workspace,
        }
    }

    /// 兑换判定（设计 §4）：Some 即拒绝；优先级 撤销 > 过期 > 他人重用 > 超次。
    pub fn deny_kind(&self, peer: &str, now_secs: u64) -> Option<ShareDenyKind> {
        if self.revoked {
            return Some(ShareDenyKind::Revoked);
        }
        if now_secs >= self.expires_at_unix {
            return Some(ShareDenyKind::Expired);
        }
        if self
            .bound_peer
            .as_deref()
            .is_some_and(|bound| bound != peer)
        {
            return Some(ShareDenyKind::Reuse);
        }
        if self.activations >= self.max_activations {
            return Some(ShareDenyKind::Exhausted);
        }
        None
    }

    /// 展示状态（设计 §8 徽章口径）：revoked/expired/exhausted/bound/active。
    pub fn status(&self, now_secs: u64) -> &'static str {
        let badges = [
            (self.revoked, "revoked"),
            (now_secs >= self.expires_at_unix, "expired"),
            (self.activations >= self.max_activations, "exhausted"),
            (self.bound_peer.is_some(), "bound"),
        ];
        badges
            .iter()
            .find(|(hit, _)| *hit)
            .map_or("active", |(_, label)| label)
    }
}

/// 兑换拒绝四分类（设计 §4/§10）：audit_key 对应五条分享审计事件之四，error_code 是 denied 帧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareDenyKind {
    Reuse,
    Expired,
    Revoked,
    Exhausted,
}

impl ShareDenyKind {
    /// 审计事件键（share-reuse-denied / share-expired / share-revoked / share-exhausted）。
    pub fn audit_key(&self) -> &'static str {
        match self {
            Self::Reuse => "share-reuse-denied",
            Self::Expired => "share-expired",
            Self::Revoked => "share-revoked",
            Self::Exhausted => "share-exhausted",
        }
    }

    /// denied 帧错误码。
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::Reuse => ErrorCode::ShareReuseDenied,
            Self::Expired => ErrorCode::ShareExpired,
            Self::Revoked => ErrorCode::ShareRevoked,
            Self::Exhausted => ErrorCode::ShareExhausted,
        }
    }
}

/// 台账文件信封：带版本号，升级路径显式。
#[derive(Debug, Serialize, Deserialize)]
struct ShareFile {
    version: u32,
    shares: BTreeMap<String, ShareEntry>,
}

/// 台账内存模型 + 文件存取（键 = share_id 字符串，BTreeMap 序输出稳定）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareLedger {
    shares: BTreeMap<String, ShareEntry>,
}

impl ShareLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, share_id: &str) -> Option<&ShareEntry> {
        self.shares.get(share_id)
    }

    pub fn get_mut(&mut self, share_id: &str) -> Option<&mut ShareEntry> {
        self.shares.get_mut(share_id)
    }

    /// 按 token 哈希定位条目 id（兑换入口；不匹配 = None，沿用既有拒绝路径）。
    pub fn find_id_by_token_hash(&self, token_hash: &str) -> Option<String> {
        self.shares
            .iter()
            .find(|(_, entry)| entry.token_sha256 == token_hash)
            .map(|(id, _)| id.clone())
    }

    pub fn insert(&mut self, entry: ShareEntry) {
        self.shares.insert(entry.share_id.to_string(), entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &ShareEntry)> {
        self.shares.iter().map(|(id, e)| (id.as_str(), e))
    }

    /// 读取台账：与策略表同口径——缺失由调用方定夺，损坏/版本不符显式报错。
    pub fn load(path: &Path) -> Result<Self, ShareStoreError> {
        let raw = std::fs::read(path)?;
        let file: ShareFile = serde_json::from_slice(&raw)?;
        if file.version != SHARES_FILE_VERSION {
            return Err(ShareStoreError::UnsupportedVersion(file.version));
        }
        Ok(Self {
            shares: file.shares,
        })
    }

    /// 原子写：先落同目录临时文件并 sync，再 rename 原子生效；失败错误上抛。
    pub fn save(&self, path: &Path) -> Result<(), ShareStoreError> {
        let json = serde_json::to_string_pretty(&ShareFile {
            version: SHARES_FILE_VERSION,
            shares: self.shares.clone(),
        })?;
        let tmp = path.with_extension("json.tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShareStoreError {
    #[error("share ledger unreadable: {0}")]
    Io(#[from] std::io::Error),
    #[error("share ledger corrupted, refusing silent fallback: {0}")]
    Corrupted(#[from] serde_json::Error),
    #[error("share ledger version {0} unsupported")]
    UnsupportedVersion(u32),
}

/// 128-bit 随机 token（32 位小写 hex）；原文只进创建响应与链接，禁落盘/日志/审计。
pub fn generate_token() -> String {
    let bytes: [u8; 16] = rand::random();
    hex_encode(&bytes)
}

/// token sha256（hex 小写）：台账唯一持留形态。
pub fn token_sha256(token: &str) -> String {
    hex_encode(&Sha256::digest(token.as_bytes()))
}

/// Unix 秒 → RFC 3339（UTC，秒级，恒 Z 后缀）。
pub fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (hour, min, sec) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// 当前 Unix 秒；缺时钟回落 epoch 不 panic。
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 链接组装（设计 §2 冻结格式）：addr 可重复；exp/sid 为展示提示，权威判定在 agent 侧。
pub fn build_share_link(
    peer: &str,
    addrs: &[String],
    token: &str,
    expires_at_unix: u64,
    share_id: &str,
) -> String {
    let mut link = format!("{SHARE_LINK_SCHEME}://v1?peer={peer}");
    for addr in addrs {
        link.push_str("&addr=");
        link.push_str(addr);
    }
    link.push_str("&token=");
    link.push_str(token);
    link.push_str("&exp=");
    link.push_str(&expires_at_unix.to_string());
    link.push_str("&sid=");
    link.push_str(share_id);
    link
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Howard Hinnant civil_from_days：Unix 天数 → (年, 月, 日)，纯整数历法换算。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day)
}
