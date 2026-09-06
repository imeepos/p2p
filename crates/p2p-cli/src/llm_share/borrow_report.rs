//! borrow 报告形态与构造（F11/PR6）：done / stream_broken / rejected 三态，
//! 全字段显式；拒绝码保持 wire snake_case 语义（机器可区分），不本地化改写。

use llm_share_ledger::WINDOW_SECS;
use llm_share_proxy::{ErrorCode, ProxyRequest};
use serde::Serialize;

use super::borrow::BorrowParams;
use super::ledger;

/// borrow 结果报告：done / stream_broken / rejected 三态，失败路径全字段显式。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BorrowReport {
    pub status: &'static str,
    pub lender: String,
    pub model: String,
    pub req_id: String,
    pub period: String,
    pub code: Option<String>,
    pub message: Option<String>,
    pub input: u64,
    pub output: u64,
    pub estimated: bool,
    /// 争议窗口秒数（A6：普通 24h，estimated 72h）。
    pub dispute_window_secs: u64,
    pub sse_frames: usize,
    pub ledger_file: String,
    /// 单笔收据 wire 文件（p2pctl llm-share receipt verify 直接可读）。
    pub receipt_file: String,
    /// 本次是否新增入账（false = req_id 已在账，幂等重放）。
    pub appended: bool,
    /// 上游 SSE 事件原文（OpenAI 流式语义原样透出）。
    pub sse: Vec<String>,
}

/// 预检拒绝报告：三闸拒绝（未授权/模型不供/冻结不足等）上游零调用、流水零产生，
/// 拒绝码 wire 语义原样透出。
pub(super) fn rejected_report(
    params: &BorrowParams,
    req: &ProxyRequest,
    code: &ErrorCode,
    message: &str,
    sse_frames: usize,
    sse: Vec<String>,
) -> BorrowReport {
    BorrowReport {
        status: "rejected",
        lender: params.lender.clone(),
        model: req.model.clone(),
        req_id: req.req_id.clone(),
        period: String::new(),
        code: Some(code_text(code)),
        message: Some(message.to_owned()),
        input: 0,
        output: 0,
        estimated: false,
        dispute_window_secs: WINDOW_SECS,
        sse_frames,
        ledger_file: ledger::path(&params.ledger_dir).display().to_string(),
        receipt_file: String::new(),
        appended: false,
        sse,
    }
}

/// ErrorCode → snake_case 文本（wire 语义原样透出，机器可区分）。
fn code_text(code: &ErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}
