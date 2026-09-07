//! 分享台账与链接生成（llm-share-link 设计 §5.4 C 段，W2）：create/list/revoke。
//! 台账落盘 <data-dir>/llm-share/shares.json（llm-share-link::ledger 原子写）；
//! token 原文只出现在返回的链接里一次，台账只存 sha256（契约 §16.6 #2）。

use std::path::PathBuf;

use llm_share_link::ledger::{LedgerError, ShareEntry, ShareLedger, SOURCE_PREFIX};
use llm_share_link::link::build_link;
use llm_share_link::token::generate_token;
use serde::Serialize;

use super::allowlist;
use super::file_path;
use super::offer;
use super::provider;
use super::read_json_or_none;
use super::validate_peer_id;

/// 分享默认有效期（秒）：24h。
pub const DEFAULT_TTL_SECS: u64 = 24 * 3600;
/// 分享有效期上限（秒）：7d。
pub const MAX_TTL_SECS: u64 = 7 * 24 * 3600;

fn ledger_path(data_dir: &str) -> PathBuf {
    file_path(data_dir, llm_share_link::ledger::FILE_NAME)
}

fn ledger_err(e: LedgerError) -> String {
    e.to_string()
}

/// share create 入参（apps/cli clap 层装配，本层校验）。
pub struct ShareCreateParams {
    pub peer: String,
    pub provider_id: String,
    /// None = provider 全模型。
    pub models: Option<Vec<String>>,
    /// None = now + DEFAULT_TTL_SECS。
    pub expires_at_unix: Option<u64>,
    pub note: String,
    pub addrs: Vec<String>,
}

/// share create 报告（token 只存在于 link 内，报告不单列 token 字段）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareCreateReport {
    pub link: String,
    pub share_id: String,
    pub expires_at: u64,
    pub models: Vec<String>,
}

/// 建分享：peer 校验 → provider 存在 → 模型（显式或 provider 全量）⊆ offer →
/// exp 边界 → 生成 token → 台账 create+save → 组装链接。
pub fn share_create(
    data_dir: &str,
    params: ShareCreateParams,
    now: u64,
) -> Result<ShareCreateReport, String> {
    validate_peer_id(&params.peer)?;
    let provider_config = provider::get(data_dir, &params.provider_id)?
        .ok_or_else(|| format!("provider 不存在：{}", params.provider_id))?;
    let models = match &params.models {
        Some(raw) => allowlist::normalize_models(raw)?,
        None => provider_config.models.clone(),
    };
    if models.is_empty() {
        return Err("分享模型不能为空（provider 未声明模型或 --model 全空）".to_owned());
    }
    let offer_file = offer::path(data_dir);
    let signed: llm_share_offer::SignedOffer = read_json_or_none(&offer_file, "能力声明")?
        .ok_or_else(|| {
            format!(
                "暂无能力声明（{}）：先 llm-share offer publish",
                offer_file.display()
            )
        })?;
    for model in &models {
        if !signed.offer.models.iter().any(|m| m == model) {
            return Err(format!(
                "模型 {model} 不在当前能力声明中（offer.models={}）",
                signed.offer.models.join(",")
            ));
        }
    }
    let expires_at = match params.expires_at_unix {
        Some(exp) => {
            if exp <= now {
                return Err(format!("--expires-at 必须晚于当前时间（now={now}）"));
            }
            if exp > now + MAX_TTL_SECS {
                return Err(format!(
                    "--expires-at 超过上限（最多 now+{MAX_TTL_SECS}s = {}）",
                    now + MAX_TTL_SECS
                ));
            }
            exp
        }
        None => now + DEFAULT_TTL_SECS,
    };
    let token = generate_token();
    let file = ledger_path(data_dir);
    let mut ledger = ShareLedger::load_or_empty(&file).map_err(ledger_err)?;
    let share_id = ledger.create(
        &token,
        &params.provider_id,
        models.clone(),
        expires_at,
        &params.note,
        now,
    );
    ledger.save(&file).map_err(ledger_err)?;
    let link = build_link(
        &params.peer,
        &params.addrs,
        &token,
        Some(expires_at),
        Some(share_id.clone()),
        Some(models.clone()),
    );
    Ok(ShareCreateReport {
        link,
        share_id,
        expires_at,
        models,
    })
}

/// share list 条目（脱敏：无 token 原文与哈希；status 由台账推导）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareListEntry {
    pub share_id: String,
    pub provider_id: String,
    pub models: Vec<String>,
    pub activations: u32,
    pub expires_at: u64,
    pub revoked: bool,
    pub bound_peer: Option<String>,
    pub note: String,
    pub created_at: u64,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareListReport {
    pub shares: Vec<ShareListEntry>,
}

/// 台账清单（created_at 升序稳定输出）。
pub fn share_list(data_dir: &str, now: u64) -> Result<ShareListReport, String> {
    let ledger = ShareLedger::load_or_empty(&ledger_path(data_dir)).map_err(ledger_err)?;
    let mut shares: Vec<ShareListEntry> = ledger
        .iter()
        .map(|(_, entry)| entry_view(entry, now))
        .collect();
    shares.sort_by(|a, b| (a.created_at, &a.share_id).cmp(&(b.created_at, &b.share_id)));
    Ok(ShareListReport { shares })
}

/// share revoke 报告（allowlist_removed = 级联删除的 share 来源条目数）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareRevokeReport {
    pub share_id: String,
    pub revoked: bool,
    pub allowlist_removed: usize,
}

/// 撤销分享：不存在/已撤销=显式报错（契约 §16.6）；置 revoked + 原子写 +
/// 按 source=share:<id> 级联删 allowlist 条目。
pub fn share_revoke(data_dir: &str, share_id: &str) -> Result<ShareRevokeReport, String> {
    let file = ledger_path(data_dir);
    let mut ledger = ShareLedger::load_or_empty(&file).map_err(ledger_err)?;
    let entry = ledger
        .get(share_id)
        .ok_or_else(|| format!("分享不存在：{share_id}"))?;
    if entry.revoked {
        return Err(format!("分享已撤销：{share_id}（契约 §16.6 显式报错）"));
    }
    ledger.revoke(share_id);
    ledger.save(&file).map_err(ledger_err)?;
    let removed = allowlist::remove_by_source(data_dir, &format!("{SOURCE_PREFIX}{share_id}"))?;
    Ok(ShareRevokeReport {
        share_id: share_id.to_owned(),
        revoked: true,
        allowlist_removed: removed,
    })
}

fn entry_view(entry: &ShareEntry, now: u64) -> ShareListEntry {
    ShareListEntry {
        share_id: entry.share_id.clone(),
        provider_id: entry.provider_id.clone(),
        models: entry.models.clone(),
        activations: entry.activations,
        expires_at: entry.expires_at_unix,
        revoked: entry.revoked,
        bound_peer: entry.bound_peer.clone(),
        note: entry.note.clone(),
        created_at: entry.created_at,
        status: entry.status(now).to_owned(),
    }
}

#[cfg(test)]
mod tests;