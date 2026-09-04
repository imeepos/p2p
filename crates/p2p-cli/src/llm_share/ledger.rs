//! 双边流水查询（§3.1/§5.1）：本机收据副本的明细与净差视图。
//! 存储文件 <data-dir>/llm-share/ledger.json（append-only 收据列表，§5.1 wire 形态），
//! 由运行面（proxy/导入）写入；本模块只读 + 原子落盘工具。
//! 净差口径对齐 llm-share-ledger::Ledger::net：lender 记正、borrower 记负，
//! 按 (lender, period) 切分（Q1：账期为出借方本地口径）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use llm_share_ledger::Receipt;
use serde::{Deserialize, Serialize};

use super::read_json_or_none;
use super::validate_peer_id;
use super::write_json_atomic;

pub const FILE_NAME: &str = "ledger.json";
const FORMAT_VERSION: u8 = 1;

/// 本机流水副本落盘形态（收据保持 §5.1 wire 字段，不做二次包装）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerFile {
    pub v: u8,
    pub receipts: Vec<Receipt>,
}

impl LedgerFile {
    pub fn new() -> Self {
        Self {
            v: FORMAT_VERSION,
            receipts: Vec::new(),
        }
    }

    /// append-only 追加；幂等由调用方按 req_id 保证（对齐账本 apply 语义）。
    pub fn append(&mut self, receipt: Receipt) {
        self.receipts.push(receipt);
    }
}

pub fn path(data_dir: &str) -> PathBuf {
    super::file_path(data_dir, FILE_NAME)
}

/// 读流水：缺失视为空账（首笔前查询合法）；损坏显式报错。
pub fn load_or_empty(path: &Path) -> Result<LedgerFile, String> {
    match read_json_or_none(path, "流水账本")? {
        Some(file) => Ok(file),
        None => Ok(LedgerFile::new()),
    }
}

pub fn save(path: &Path, file: &LedgerFile) -> Result<(), String> {
    write_json_atomic(path, file, "流水账本")
}

/// 流水明细视图（camelCase 输出契约；tokens = input + output）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntryView {
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

/// list 报告。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerListReport {
    pub count: usize,
    pub entries: Vec<LedgerEntryView>,
}

/// list 过滤器（None = 不过滤）。
#[derive(Debug, Default, Clone, Copy)]
pub struct LedgerFilters<'a> {
    pub lender: Option<&'a str>,
    pub borrower: Option<&'a str>,
    pub period: Option<&'a str>,
}

/// list 主流程：过滤 → 明细（存储序，append-only 时间正序）。
pub fn list(data_dir: &str, filters: LedgerFilters) -> Result<LedgerListReport, String> {
    let file = load_or_empty(&path(data_dir))?;
    let matches = |field: Option<&str>, value: &str| field.is_none_or(|want| want == value);
    let entries: Vec<LedgerEntryView> = file
        .receipts
        .iter()
        .filter(|r| matches(filters.lender, &r.lender))
        .filter(|r| matches(filters.borrower, &r.borrower))
        .filter(|r| matches(filters.period, &r.period))
        .map(view_of)
        .collect();
    Ok(LedgerListReport {
        count: entries.len(),
        entries,
    })
}

/// 净差行：本机为 lender 的借出合计、为 borrower 的借入合计与净差。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceRow {
    pub lender: String,
    pub period: String,
    pub lent_out: u64,
    pub borrowed: u64,
    pub net: i64,
    pub entries: usize,
}

/// balance 报告：本机视角（self_peer 参与的双边条目）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceReport {
    pub peer: String,
    pub rows: Vec<BalanceRow>,
}

/// balance 主流程：按 (lender, period) 聚合，符号规则对齐账本 net 视图。
pub fn balance(
    data_dir: &str,
    self_peer: &str,
    period: Option<&str>,
) -> Result<BalanceReport, String> {
    validate_peer_id(self_peer)?;
    let file = load_or_empty(&path(data_dir))?;
    let mut groups: BTreeMap<(String, String), (u64, u64, usize)> = BTreeMap::new();
    for receipt in &file.receipts {
        if let Some(want) = period {
            if receipt.period != want {
                continue;
            }
        }
        let as_lender = receipt.lender == self_peer;
        let as_borrower = receipt.borrower == self_peer;
        if !as_lender && !as_borrower {
            continue;
        }
        let tokens = receipt.usage.input.saturating_add(receipt.usage.output);
        let entry = groups
            .entry((receipt.lender.clone(), receipt.period.clone()))
            .or_default();
        if as_lender {
            entry.0 = entry.0.saturating_add(tokens);
        } else {
            entry.1 = entry.1.saturating_add(tokens);
        }
        entry.2 += 1;
    }
    Ok(BalanceReport {
        peer: self_peer.to_owned(),
        rows: groups
            .into_iter()
            .map(
                |((lender, period), (lent_out, borrowed, entries))| BalanceRow {
                    lender,
                    period,
                    lent_out,
                    borrowed,
                    net: lent_out as i64 - borrowed as i64,
                    entries,
                },
            )
            .collect(),
    })
}

fn view_of(receipt: &Receipt) -> LedgerEntryView {
    LedgerEntryView {
        req_id: receipt.req_id.clone(),
        period: receipt.period.clone(),
        lender: receipt.lender.clone(),
        borrower: receipt.borrower.clone(),
        model: receipt.model.clone(),
        input: receipt.usage.input,
        output: receipt.usage.output,
        tokens: receipt.usage.input.saturating_add(receipt.usage.output),
        estimated: receipt.estimated,
        ts: receipt.ts,
    }
}

#[cfg(test)]
mod tests;
