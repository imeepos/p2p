//! 能力声明签发/查看（§5.2/§7.3）：字段齐备（模型/余量/账期末/TTL/rate-limit/retention），
//! 复用 llm-share-offer 的 Offer/SignedOffer（canonical JSON + Ed25519，peer 绑定 + TTL 窗口）。
//! 签名密钥 = 节点身份种子（p2p_identity::load_seed，0600 标准，缺失显式报错不代生成）；
//! 声明信封是公开数据，落盘 <data-dir>/llm-share/offer.json 原子写。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use llm_share_offer::{Offer, OfferError, RateLimit, SignedOffer, VerifyError};
use serde::Serialize;

use super::file_path;
use super::pairs_to_map;
use super::parse_model_u64;
use super::read_json_or_none;
use super::validate_date_ymd;
use super::write_json_atomic;

pub const FILE_NAME: &str = "offer.json";

/// publish 参数（apps/cli clap 层装配，本层校验）。
pub struct OfferParams {
    pub models: Vec<String>,
    /// --spare model=N 键值原文。
    pub spare: Vec<String>,
    /// 账期截止日 YYYY-MM-DD。
    pub period_ends: String,
    /// --max-per-req model=N 键值原文（可空）。
    pub max_per_req: Vec<String>,
    pub rpm: u32,
    pub concurrency: u32,
    pub ttl_secs: u64,
    /// 数据留存自述；None 回落 "none"（§7.3 默认不落盘 prompt）。
    pub retention: Option<String>,
}

/// 声明报告（publish/show 共用事实源；ttl/rate_limit 沿 §5.2 wire 字段名）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferReport {
    pub peer: String,
    pub models: Vec<String>,
    pub spare: BTreeMap<String, u64>,
    pub period_ends: String,
    pub max_per_req: BTreeMap<String, u64>,
    pub rate_limit: RateLimit,
    pub ttl: u64,
    pub retention: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub file: String,
}

/// show 报告：声明本体 + 剩余 TTL 与生效状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferShowReport {
    #[serde(flatten)]
    pub offer: OfferReport,
    pub remaining_secs: i64,
    pub status: String,
}

pub fn path(data_dir: &str) -> PathBuf {
    file_path(data_dir, FILE_NAME)
}

/// publish 主流程：载种子 → 组装校验 → 签名 → 原子落盘 → 报告。
pub fn publish(
    seed_path: &Path,
    data_dir: &str,
    params: &OfferParams,
    issued_at: u64,
) -> Result<OfferReport, String> {
    let keypair = p2p_identity::load_seed(seed_path).map_err(|e| {
        format!(
            "节点身份加载失败（{}）: {e}；声明必须以本机身份签发",
            seed_path.display()
        )
    })?;
    let offer = build_offer(keypair.peer_id().to_string().as_str(), params)?;
    let signed =
        SignedOffer::sign(&offer, &keypair, issued_at).map_err(|e| offer_error_text(&e))?;
    let file = path(data_dir);
    write_json_atomic(&file, &signed, "能力声明")?;
    Ok(report_of(&signed, &file))
}

/// show 主流程：读信封 → 验签定状态 → 剩余 TTL（可为负，即已过期秒数）。
pub fn show(data_dir: &str, now: u64) -> Result<OfferShowReport, String> {
    let file = path(data_dir);
    let signed: SignedOffer = match read_json_or_none(&file, "能力声明")? {
        Some(signed) => signed,
        None => {
            return Err(format!(
                "暂无能力声明（{}）：先 llm-share offer publish",
                file.display()
            ))
        }
    };
    let status = match signed.verify(now) {
        Ok(()) => "live",
        Err(VerifyError::Expired(_)) => "expired",
        Err(VerifyError::NotYetValid) => "not_yet_valid",
        Err(VerifyError::PeerMismatch) => "peer_mismatch",
        Err(VerifyError::BadSignature) => "bad_signature",
        Err(VerifyError::Encoding) => "encoding",
    };
    let remaining = signed.expires_at() as i64 - now as i64;
    Ok(OfferShowReport {
        offer: report_of(&signed, &file),
        remaining_secs: remaining,
        status: status.to_owned(),
    })
}

/// 组装 + 预校验（模型非空去重、账期日期、rate-limit/TTL 正值）；
/// spare 覆盖与关联键合法性由 Offer::validate 兜底（见 offer_error_text 映射）。
fn build_offer(peer: &str, params: &OfferParams) -> Result<Offer, String> {
    let mut models: Vec<String> = Vec::new();
    for value in &params.models {
        let model = value.trim();
        if model.is_empty() {
            return Err("--model 模型名不能为空".to_owned());
        }
        if !models.iter().any(|known| known == model) {
            models.push(model.to_owned());
        }
    }
    validate_date_ymd(&params.period_ends)?;
    if params.rpm == 0 || params.concurrency == 0 {
        return Err("--rpm/--concurrency 必须为正整数".to_owned());
    }
    if params.ttl_secs == 0 {
        return Err("--ttl 必须为正整数秒".to_owned());
    }
    let spare = pairs_to_map(parse_model_u64(&params.spare, "--spare")?, "--spare")?;
    let max_per_req = pairs_to_map(
        parse_model_u64(&params.max_per_req, "--max-per-req")?,
        "--max-per-req",
    )?;
    Ok(Offer {
        peer: peer.to_owned(),
        models,
        spare,
        period_ends: params.period_ends.clone(),
        max_per_req,
        rate_limit: RateLimit {
            rpm: params.rpm,
            concurrency: params.concurrency,
        },
        ttl_secs: params.ttl_secs,
        retention: params
            .retention
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
    })
}

/// OfferError → 可读中文错误（声明校验失败的显式失败路径）。
fn offer_error_text(e: &OfferError) -> String {
    match e {
        OfferError::Empty(field) => format!("声明字段 {field} 不能为空"),
        OfferError::SparePositive(model) => {
            format!("--spare 缺少模型 {model} 的正闲量声明（零闲量等于没这能力）")
        }
        OfferError::UnknownModel(model) => {
            format!("--spare/--max-per-req 引用了未声明的模型 {model}")
        }
        OfferError::Positive(field) => format!("声明字段 {field} 必须为正"),
        OfferError::Encoding(msg) => format!("声明序列化失败: {msg}"),
    }
}

/// 信封 → 报告事实源。
fn report_of(signed: &SignedOffer, file: &Path) -> OfferReport {
    OfferReport {
        peer: signed.offer.peer.clone(),
        models: signed.offer.models.clone(),
        spare: signed.offer.spare.clone(),
        period_ends: signed.offer.period_ends.clone(),
        max_per_req: signed.offer.max_per_req.clone(),
        rate_limit: signed.offer.rate_limit,
        ttl: signed.offer.ttl_secs,
        retention: signed.offer.retention.clone(),
        issued_at: signed.issued_at,
        expires_at: signed.expires_at(),
        file: file.display().to_string(),
    }
}

#[cfg(test)]
mod tests;
