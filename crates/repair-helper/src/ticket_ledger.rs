//! 工单存活表：一次性票据查重（remote-support-plan.md §3.3）与惰性焚毁。
//!
//! 语义：claim 登记 ticket_id -> exp；登记时先惰性清除已过期条目（过期即焚）；
//! 进程停机即焚（纯内存，无落盘——P0b helper 单工单生命周期）；同 id 未过期
//! 再次 claim 拒绝（AlreadyUsed）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ticket::TicketError;

/// 一次性票据登记表（线程安全，Clone 共享同一张表）。
#[derive(Clone, Default)]
pub struct TicketLedger {
    inner: Arc<Mutex<HashMap<String, u64>>>,
}

impl TicketLedger {
    /// 登记票据：先焚毁已过期条目，再查重插入。锁中毒上抛原因不静默。
    pub fn claim(&self, ticket_id: &str, exp: u64, now_unix: u64) -> Result<(), TicketError> {
        let mut map = self
            .inner
            .lock()
            .map_err(|_| TicketError::Malformed("ledger lock poisoned".to_string()))?;
        map.retain(|_, stored_exp| *stored_exp > now_unix);
        if map.contains_key(ticket_id) {
            return Err(TicketError::AlreadyUsed);
        }
        map.insert(ticket_id.to_string(), exp);
        Ok(())
    }

    /// 存活票据数（测试/观测）。
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_is_one_time_within_validity() {
        let ledger = TicketLedger::default();
        ledger.claim("t1", 200, 100).unwrap();
        assert_eq!(ledger.claim("t1", 200, 100), Err(TicketError::AlreadyUsed));
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn expired_entries_are_burned_lazily() {
        let ledger = TicketLedger::default();
        ledger.claim("t1", 150, 100).unwrap();
        // 同一 id 在过期后再 claim：先焚毁旧条目，因此放行
        ledger.claim("t1", 300, 200).unwrap();
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn distinct_ids_coexist() {
        let ledger = TicketLedger::default();
        ledger.claim("a", 200, 100).unwrap();
        ledger.claim("b", 200, 100).unwrap();
        assert_eq!(ledger.len(), 2);
    }
}
