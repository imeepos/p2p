//! append-only 双边流水账本（§3.1/§5.1）：req_id 幂等去重，余额是视图，哈希链支撑对账。
//! estimated 收据争议窗口（Q3：普通 24h、estimated 72h）同属收据入账生命周期，一并在此承载。

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

use crate::error::{Error, Result};
use crate::receipt::Receipt;

/// 账本条目：中立镜像收据，出借方视角贷方正、使用方视角借方负；hash 链首条前驱为全零。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub seq: u64,
    pub receipt: Receipt,
    pub hash: [u8; 32],
}

impl Entry {
    fn seal(seq: u64, receipt: &Receipt, prev: [u8; 32]) -> Result<Self> {
        let mut hasher = Sha256::new();
        hasher.update(prev);
        hasher.update(seq.to_be_bytes());
        hasher.update(receipt.canonical_payload()?);
        Ok(Self { seq, receipt: receipt.clone(), hash: hasher.finalize().into() })
    }
}

/// 本地全量流水副本（append-only）。apply 是唯一写入口：先验签，再按 req_id 去重，最后入链。
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    entries: Vec<Entry>,
    seen: HashSet<String>,
}

impl Ledger {
    /// 收据入账。Ok(false) 表示 req_id 重放只记一笔（MVP A4）；验签失败拒绝入账。
    pub fn apply(&mut self, receipt: &Receipt, lender_pubkey: &[u8; 32]) -> Result<bool> {
        receipt.verify(lender_pubkey)?;
        if !self.seen.insert(receipt.req_id.clone()) {
            return Ok(false);
        }
        let prev = self.entries.last().map(|e| e.hash).unwrap_or([0u8; 32]);
        let entry = Entry::seal(self.entries.len() as u64, receipt, prev)?;
        self.entries.push(entry);
        Ok(true)
    }

    /// 净差视图（双边记账：peer 为出借方记正、为使用方记负）；period 为出借方本地账期（Q1）。
    pub fn net(&self, peer: &str, period: &str) -> i64 {
        self.entries
            .iter()
            .filter(|e| e.receipt.period == period)
            .filter_map(|e| {
                let t = (e.receipt.usage.input + e.receipt.usage.output) as i64;
                (e.receipt.lender == peer)
                    .then_some(t)
                    .or((e.receipt.borrower == peer).then_some(-t))
            })
            .sum()
    }

    /// 对账：逐 seq 比对双方条目哈希；哈希覆盖全字段与链位，任何一侧篡改即分叉（§5.1）。
    pub fn reconcile(&self, remote: &Ledger) -> ReconReport {
        let mut report = ReconReport::default();
        for i in 0..self.entries.len().max(remote.entries.len()) {
            match (self.entries.get(i), remote.entries.get(i)) {
                (Some(a), Some(b)) if a.hash == b.hash => report.matched += 1,
                (a, b) => {
                    report.local_only.extend(a.iter().map(|e| e.seq));
                    report.remote_only.extend(b.iter().map(|e| e.seq));
                }
            }
        }
        report
    }
}

/// 对账结论：matched 为双方一致的前缀条数，分歧与缺失条目按各自 seq 列出。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconReport {
    pub matched: usize,
    pub local_only: Vec<u64>,
    pub remote_only: Vec<u64>,
}

/// 争议窗口秒数：前者普通收据 24h，后者 estimated 收据 72h（Q3 裁决）。
pub const WINDOW_SECS: u64 = 24 * 3600;
pub const WINDOW_ESTIMATED_SECS: u64 = 72 * 3600;

/// 争议状态机：Pending（待争议）-> Disputed（已争议）| Finalized（终局入账）。
#[derive(Debug, Default)]
pub struct DisputeTracker {
    slots: HashMap<String, (SlotState, u64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    Pending,
    Disputed,
    Finalized,
}

impl DisputeTracker {
    /// 收据进入观察期；窗口自收据 ts 起算，estimated 走 72h 档。
    pub fn track(&mut self, req_id: &str, estimated: bool, ts: u64) {
        let window = if estimated { WINDOW_ESTIMATED_SECS } else { WINDOW_SECS };
        self.slots.insert(req_id.to_string(), (SlotState::Pending, ts + window));
    }

    /// 窗口内提出争议；超窗、已争议或已终局拒绝。
    pub fn dispute(&mut self, req_id: &str, now: u64) -> Result<()> {
        let slot = self.slot(req_id)?;
        if now > slot.1 || slot.0 != SlotState::Pending {
            return Err(Error::DisputeWindow(req_id.to_string()));
        }
        slot.0 = SlotState::Disputed;
        Ok(())
    }

    /// 终局入账：窗口届满且无争议；未届满、争议未决或重复终局拒绝。
    pub fn finalize(&mut self, req_id: &str, now: u64) -> Result<()> {
        let slot = self.slot(req_id)?;
        if slot.0 != SlotState::Pending || now <= slot.1 {
            return Err(Error::DisputeWindow(req_id.to_string()));
        }
        slot.0 = SlotState::Finalized;
        Ok(())
    }

    fn slot(&mut self, req_id: &str) -> Result<&mut (SlotState, u64)> {
        self.slots.get_mut(req_id).ok_or_else(|| Error::NotFound(req_id.into()))
    }
}
