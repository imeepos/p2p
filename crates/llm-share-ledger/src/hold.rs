//! 预授权冻结（§4 步骤 3/7、§7.1）：冻结是硬闸——无冻结不结算，净差超限结构化拒绝。

use std::collections::HashMap;

use crate::error::{Error, Result};

/// 净差上限策略（轮 52 Q2）：上限 = 声明 spare 的万分比（默认 5000 即 50%），可叠加绝对封顶。
#[derive(Debug, Clone)]
pub struct LimitPolicy {
    pub spare_ratio_bps: u64,
    pub absolute_cap: Option<u64>,
}

impl Default for LimitPolicy {
    fn default() -> Self {
        Self { spare_ratio_bps: 5000, absolute_cap: None }
    }
}

impl LimitPolicy {
    pub fn limit(&self, spare: u64) -> u64 {
        (spare.saturating_mul(self.spare_ratio_bps) / 10_000)
            .min(self.absolute_cap.unwrap_or(u64::MAX))
    }
}

/// 冻结申请：预授权阶段双方已知的 req_id/角色/账期与估算上限，结算凭其字段构建收据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreezeRequest {
    pub req_id: String,
    pub lender: String,
    pub borrower: String,
    pub period: String,
    pub est: u64,
}

/// 冻结账。req_id 幂等：重复 freeze 直接成功不重复占额；settle/release 消费后移除。
#[derive(Debug, Default)]
pub struct HoldManager {
    holds: HashMap<String, FreezeRequest>,
}

impl HoldManager {
    /// 硬闸第一道：净差 + 在途冻结 + 本次估算超上限即拒绝，上游零调用（MVP A2）。
    pub fn freeze(&mut self, limit: u64, ledger_net: i64, req: FreezeRequest) -> Result<()> {
        if self.holds.contains_key(&req.req_id) {
            return Ok(());
        }
        let cut = self.holds.values().filter(|h| h.lender == req.lender && h.period == req.period);
        let open = cut.map(|h| h.est).sum::<u64>().saturating_add(req.est);
        let projected = ledger_net.saturating_add(open as i64);
        if req.est == 0 || projected > limit.min(i64::MAX as u64) as i64 {
            return Err(Error::NetDiffExceeded(limit, projected.max(0) as u64));
        }
        self.holds.insert(req.req_id.clone(), req);
        Ok(())
    }

    /// 硬闸第二道：按实际 usage 结算并解除冻结；usage 超 est 拒绝且冻结保留。
    pub fn settle(&mut self, req_id: &str, usage: u64) -> Result<()> {
        let hold = self.holds.remove(req_id);
        let hold = hold.ok_or_else(|| Error::NotFound(req_id.into()))?;
        if usage > hold.est {
            let est = hold.est;
            self.holds.insert(req_id.to_string(), hold);
            return Err(Error::HoldInsufficient(est, usage));
        }
        Ok(())
    }

    /// 上游失败时解冻，不产生流水（§4 错误透传）。
    pub fn release(&mut self, req_id: &str) -> Result<()> {
        let hold = self.holds.remove(req_id);
        hold.map(drop).ok_or_else(|| Error::NotFound(req_id.into()))
    }
}
