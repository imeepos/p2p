//! 分享台账：shares.json（v1 信封，tmp+rename 原子写）；兑换激活锁内 read-check-write
//! 防 TOCTOU（设计 §5.4 C 段 + 契约 §16.6 #3）。token 只存 sha256，原文不落盘。

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::token::token_sha256;

/// 台账文件名（落 <app-data>/llm-share/ 下）。
pub const FILE_NAME: &str = "shares.json";
pub const FORMAT_VERSION: u8 = 1;
/// allowlist 来源前缀：source=share:<shareId>。
pub const SOURCE_PREFIX: &str = "share:";
/// 一次性激活固定 1（v1 不做 N 个不同 peer 各激活一次）。
pub const MAX_ACTIVATIONS: u32 = 1;

/// 单条分享（token 原文永不落盘）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareEntry {
    pub share_id: String,
    pub token_sha256: String,
    pub provider_id: String,
    pub models: Vec<String>,
    pub max_activations: u32,
    #[serde(default)]
    pub activations: u32,
    pub expires_at_unix: u64,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub note: String,
    pub created_at: u64,
    /// 首激活绑定的借方 PeerId；同 peer 二次兑换幂等成功不计数。
    #[serde(default)]
    pub bound_peer: Option<String>,
}

impl ShareEntry {
    /// 当前状态徽章：revoked / expired / exhausted / active。
    pub fn status(&self, now: u64) -> &'static str {
        if self.revoked {
            "revoked"
        } else if now >= self.expires_at_unix {
            "expired"
        } else if self.activations >= self.max_activations {
            "exhausted"
        } else {
            "active"
        }
    }
}

/// 兑换产出（allowlist 写入由装配方注入回调执行，本 crate 不直接写文件）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedeemResult {
    pub peer: String,
    pub models: Vec<String>,
    /// source=share:<shareId>。
    pub source: String,
    pub expires_at: u64,
}

/// 兑换业务拒绝（wire_code 与 redeem 帧一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedeemError {
    Invalid,
    Revoked,
    Expired,
    Exhausted,
    BoundOther,
}

impl RedeemError {
    /// wire 拒绝码（对齐契约 §16.6 kebab-case）。
    pub fn wire_code(&self) -> &'static str {
        match self {
            RedeemError::Invalid => "invalid",
            RedeemError::Revoked => "share-revoked",
            RedeemError::Expired => "expired",
            RedeemError::Exhausted => "exhausted",
            RedeemError::BoundOther => "bound-other",
        }
    }
}

/// 台账 IO/格式错误：缺失=空账；损坏/版本不符显式报错禁止静默。
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("分享台账 IO 失败（{path}）: {err}")]
    Io { path: String, err: std::io::Error },
    #[error("分享台账损坏（{path}）: {err}")]
    Corrupted {
        path: String,
        err: serde_json::Error,
    },
    #[error("分享台账版本不支持（v{0}，当前 v{FORMAT_VERSION}）")]
    UnsupportedVersion(u8),
}

/// 落盘信封（版本 + 条目表；BTreeMap 键序稳定可复现）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShareFile {
    v: u8,
    entries: BTreeMap<String, ShareEntry>,
}

/// 分享台账（BTreeMap<share_id, ShareEntry>）。
#[derive(Debug, Default)]
pub struct ShareLedger {
    entries: BTreeMap<String, ShareEntry>,
}

impl ShareLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, share_id: &str) -> Option<&ShareEntry> {
        self.entries.get(share_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ShareEntry)> {
        self.entries.iter()
    }

    pub fn insert(&mut self, entry: ShareEntry) {
        self.entries.insert(entry.share_id.clone(), entry);
    }

    /// 建分享：uuid share_id，只存 token 摘要；返回 share_id。
    pub fn create(
        &mut self,
        token: &str,
        provider_id: &str,
        models: Vec<String>,
        expires_at_unix: u64,
        note: &str,
        created_at: u64,
    ) -> String {
        let share_id = uuid::Uuid::new_v4().to_string();
        let entry = ShareEntry {
            share_id: share_id.clone(),
            token_sha256: token_sha256(token),
            provider_id: provider_id.to_owned(),
            models,
            max_activations: MAX_ACTIVATIONS,
            activations: 0,
            expires_at_unix,
            revoked: false,
            note: note.to_owned(),
            created_at,
            bound_peer: None,
        };
        self.insert(entry);
        share_id
    }

    /// 兑换激活（锁内 read-check-write，调用方持互斥）。语义次序：
    /// 不存在→Invalid；已撤销→Revoked；过期→Expired；绑定者=本 peer→幂等成功不计数；
    /// 绑定者=其它→BoundOther；激活满→Exhausted；否则激活+1 并绑定首 peer。
    pub fn redeem(
        &mut self,
        token: &str,
        peer: &str,
        now: u64,
    ) -> Result<RedeemResult, RedeemError> {
        let digest = token_sha256(token);
        let entry = self
            .entries
            .values_mut()
            .find(|entry| entry.token_sha256 == digest)
            .ok_or(RedeemError::Invalid)?;
        if entry.revoked {
            return Err(RedeemError::Revoked);
        }
        if now >= entry.expires_at_unix {
            return Err(RedeemError::Expired);
        }
        match &entry.bound_peer {
            Some(bound) if bound == peer => {
                return Ok(redeem_result(entry));
            }
            Some(_) => return Err(RedeemError::BoundOther),
            None => {}
        }
        if entry.activations >= entry.max_activations {
            return Err(RedeemError::Exhausted);
        }
        entry.activations += 1;
        entry.bound_peer = Some(peer.to_owned());
        Ok(redeem_result(entry))
    }

    /// 置 revoked（条目保留供 list 展示）；false = share_id 不存在。
    pub fn revoke(&mut self, share_id: &str) -> bool {
        match self.entries.get_mut(share_id) {
            Some(entry) => {
                entry.revoked = true;
                true
            }
            None => false,
        }
    }

    /// 读台账：文件缺失=空账；读取/解析失败/版本不符=显式错误。
    pub fn load_or_empty(path: &Path) -> Result<Self, LedgerError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(LedgerError::Io {
                    path: path.display().to_string(),
                    err: e,
                })
            }
        };
        let file: ShareFile =
            serde_json::from_str(&text).map_err(|err| LedgerError::Corrupted {
                path: path.display().to_string(),
                err,
            })?;
        if file.v != FORMAT_VERSION {
            return Err(LedgerError::UnsupportedVersion(file.v));
        }
        Ok(Self {
            entries: file.entries,
        })
    }

    /// 原子落盘（tmp+rename，失败清理临时文件）。
    pub fn save(&self, path: &Path) -> Result<(), LedgerError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| LedgerError::Io {
                path: parent.display().to_string(),
                err,
            })?;
        }
        let file = ShareFile {
            v: FORMAT_VERSION,
            entries: self.entries.clone(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(|err| LedgerError::Io {
            path: path.display().to_string(),
            err: std::io::Error::new(std::io::ErrorKind::Other, err),
        })?;
        let tmp = path.with_extension("json.tmp");
        let written = std::fs::write(&tmp, &text).and_then(|()| std::fs::rename(&tmp, path));
        if let Err(err) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(LedgerError::Io {
                path: path.display().to_string(),
                err,
            });
        }
        Ok(())
    }
}

fn redeem_result(entry: &ShareEntry) -> RedeemResult {
    RedeemResult {
        peer: entry.bound_peer.clone().unwrap_or_default(),
        models: entry.models.clone(),
        source: format!("{SOURCE_PREFIX}{}", entry.share_id),
        expires_at: entry.expires_at_unix,
    }
}

#[cfg(test)]
mod tests;
