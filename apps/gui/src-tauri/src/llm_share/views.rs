//! 契约 §16.1 serde 视图：命名 camelCase、形状逐字（语义真值源 ai-guide 九条目；
//! 拒绝码 wire snake_case 原样透出不本地化改写，§16.2.1）。IPC 入参（publish/borrow/filter）
//! 也定义于此，flows 消费、commands 只做 State 抽取。

use std::collections::BTreeMap;

use llm_share_offer::RateLimit;
use p2p_cli::llm_share::{
    allowlist::{AllowlistEntry, AllowlistReport},
    borrow_report::BorrowReport,
    ledger::{BalanceRow, LedgerEntryView},
    offer::{OfferReport, OfferShowReport},
    receipt::ReceiptVerifyReport,
};
use serde::{Deserialize, Serialize};

/// offer 生效状态五态（§16.1：expired/not_yet_valid 常态中性；
/// peer_mismatch/bad_signature 醒目警示）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmOfferStatus {
    Live,
    Expired,
    NotYetValid,
    PeerMismatch,
    BadSignature,
}

/// LlmOfferView（§16.1）：publish 时 status/remainingSecs 缺省（对齐 ai-guide
/// publish --json 形状），show 时带五态与剩余 TTL（可为负）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmOfferView {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_secs: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LlmOfferStatus>,
}

impl From<&OfferReport> for LlmOfferView {
    fn from(r: &OfferReport) -> Self {
        Self {
            peer: r.peer.clone(),
            models: r.models.clone(),
            spare: r.spare.clone(),
            period_ends: r.period_ends.clone(),
            max_per_req: r.max_per_req.clone(),
            rate_limit: r.rate_limit,
            ttl: r.ttl,
            retention: r.retention.clone(),
            issued_at: r.issued_at,
            expires_at: r.expires_at,
            file: r.file.clone(),
            remaining_secs: None,
            status: None,
        }
    }
}

impl LlmOfferView {
    /// show 报告 → 视图：CLI 文本层 status 注入五态；encoding（信封不可规范化）属
    /// 损坏路径，按显式 Err 上抛而非伪造状态。
    pub fn from_show(report: OfferShowReport) -> Result<Self, String> {
        let mut view = Self::from(&report.offer);
        view.remaining_secs = Some(report.remaining_secs);
        view.status = Some(match report.status.as_str() {
            "live" => LlmOfferStatus::Live,
            "expired" => LlmOfferStatus::Expired,
            "not_yet_valid" => LlmOfferStatus::NotYetValid,
            "peer_mismatch" => LlmOfferStatus::PeerMismatch,
            "bad_signature" => LlmOfferStatus::BadSignature,
            other => return Err(format!("能力声明信封校验异常（{other}）：信封可能已损坏")),
        });
        Ok(view)
    }
}

/// allowlist 单条目（§16.1 LlmAllowEntry）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmAllowEntry {
    pub peer_id: String,
    pub models: Vec<String>,
    pub note: String,
    pub granted_at: String,
}

/// allowlist 视图（§16.1：allow_list 返回 { entries: [...] }，allow/deny 返回变更后视图）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmAllowlistView {
    pub entries: Vec<LlmAllowEntry>,
}

impl From<AllowlistReport> for LlmAllowlistView {
    fn from(r: AllowlistReport) -> Self {
        Self {
            entries: r
                .peers
                .into_iter()
                .map(|e: AllowlistEntry| LlmAllowEntry {
                    peer_id: e.peer_id,
                    models: e.models,
                    note: e.note,
                    granted_at: e.granted_at,
                })
                .collect(),
        }
    }
}

/// 流水明细（§16.1 LlmLedgerEntry，storage 序）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmLedgerEntry {
    pub req_id: String,
    pub period: String,
    pub lender: String,
    pub borrower: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub tokens: u64,
    pub estimated: bool,
    pub ts: u64,
}

impl From<LedgerEntryView> for LlmLedgerEntry {
    fn from(v: LedgerEntryView) -> Self {
        Self {
            req_id: v.req_id,
            period: v.period,
            lender: v.lender,
            borrower: v.borrower,
            model: v.model,
            input: v.input,
            output: v.output,
            tokens: v.tokens,
            estimated: v.estimated,
            ts: v.ts,
        }
    }
}

/// 净差方向（§16.1：正负号=借贷方向）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmBalanceDirection {
    LentOut,
    Borrowed,
    Even,
}

/// 净差分组行（§16.1 LlmBalanceGroup）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmBalanceGroup {
    pub lender: String,
    pub period: String,
    pub net_amount: i64,
    pub direction: LlmBalanceDirection,
}

impl From<BalanceRow> for LlmBalanceGroup {
    fn from(r: BalanceRow) -> Self {
        Self {
            lender: r.lender,
            period: r.period,
            net_amount: r.net,
            direction: match r.net {
                n if n > 0 => LlmBalanceDirection::LentOut,
                n if n < 0 => LlmBalanceDirection::Borrowed,
                _ => LlmBalanceDirection::Even,
            },
        }
    }
}

/// borrow 顶层状态三态（§16.1：rejected 是业务结果非命令 Err）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmBorrowStatus {
    Done,
    StreamBroken,
    Rejected,
}

/// 收据摘要（§16.1 LlmBorrowReport.receipt）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmBorrowReceipt {
    pub req_id: String,
    pub appended: bool,
    pub estimated: bool,
    pub dispute_window_secs: u64,
}

/// usage 摘要（§16.1 usage?；rejected 无 usage）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmUsage {
    pub input: u64,
    pub output: u64,
}

/// LlmBorrowReport（§16.1 逐字段：status/receipt/sseCount/usage?/code?/message?；
/// §16.2.6 sse 原文只出 sseCount）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmBorrowReport {
    pub status: LlmBorrowStatus,
    pub receipt: LlmBorrowReceipt,
    pub sse_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<LlmUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl From<BorrowReport> for LlmBorrowReport {
    fn from(r: BorrowReport) -> Self {
        let status = match r.status {
            "done" => LlmBorrowStatus::Done,
            "stream_broken" => LlmBorrowStatus::StreamBroken,
            _ => LlmBorrowStatus::Rejected,
        };
        let rejected = status == LlmBorrowStatus::Rejected;
        Self {
            status,
            receipt: LlmBorrowReceipt {
                req_id: r.req_id,
                appended: r.appended,
                estimated: r.estimated,
                dispute_window_secs: r.dispute_window_secs,
            },
            sse_count: r.sse_frames,
            usage: if rejected {
                None
            } else {
                Some(LlmUsage {
                    input: r.input,
                    output: r.output,
                })
            },
            code: r.code,
            message: r.message,
        }
    }
}

/// 收据离线验签结果（§16.1 LlmReceiptVerifyResult；verdict PASS/FAIL，
/// FAIL 是业务结果非命令 Err）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmReceiptVerifyResult {
    pub verdict: String,
    pub reason: String,
    pub req_id: String,
    pub period: String,
    pub lender: String,
    pub borrower: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub estimated: bool,
    pub ts: u64,
}

impl From<ReceiptVerifyReport> for LlmReceiptVerifyResult {
    fn from(r: ReceiptVerifyReport) -> Self {
        Self {
            verdict: r.verdict,
            reason: r.reason,
            req_id: r.req_id,
            period: r.period,
            lender: r.lender,
            borrower: r.borrower,
            model: r.model,
            input: r.input,
            output: r.output,
            estimated: r.estimated,
            ts: r.ts,
        }
    }
}
