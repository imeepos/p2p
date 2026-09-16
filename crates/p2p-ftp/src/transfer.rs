//! 一次性传输令牌登记簿。
//!
//! 令牌流：控制会话对每次传输签发随机令牌（issue）并以 oneshot 等结果；
//! 数据流入站出示令牌（take）执行传输后经 oneshot 回传结果，控制会话
//! 据此应答 226/426。令牌与签发节点绑定，过期作废（签发时惰性清理）。

use std::collections::HashMap;
use std::io;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use p2p_identity::PeerId;
use rand::RngCore;
use tokio::sync::oneshot;

use crate::wire::TOKEN_LEN;

/// 传输类型（与数据操作码一一对应，APPE 复用 PUT 操作码 + append 语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    Get,
    Put,
    Append,
    List,
    Nlst,
}

impl DataKind {
    pub fn op(self) -> u8 {
        match self {
            DataKind::Get => crate::wire::DATA_OP_GET,
            DataKind::Put | DataKind::Append => crate::wire::DATA_OP_PUT,
            DataKind::List => crate::wire::DATA_OP_LIST,
            DataKind::Nlst => crate::wire::DATA_OP_NLST,
        }
    }
}

pub(crate) struct PendingTransfer {
    pub(crate) peer: PeerId,
    pub(crate) kind: DataKind,
    pub(crate) vpath: String,
    pub(crate) issued_at: Instant,
    pub(crate) done: oneshot::Sender<io::Result<u64>>,
}

/// 登记簿本体（FtpServer 持有，会话/数据流经访问器使用）。
pub(crate) struct TransferRegistry {
    inner: Mutex<HashMap<[u8; TOKEN_LEN], PendingTransfer>>,
    token_ttl: Duration,
}

impl TransferRegistry {
    pub(crate) fn new(token_ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            token_ttl,
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<[u8; TOKEN_LEN], PendingTransfer>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 签发新令牌并登记结果通道；顺带清理过期令牌。
    pub(crate) fn issue(
        &self,
        peer: PeerId,
        kind: DataKind,
        vpath: &str,
    ) -> ([u8; TOKEN_LEN], oneshot::Receiver<io::Result<u64>>) {
        let mut guard = self.lock();
        guard.retain(|_, p| p.issued_at.elapsed() < self.token_ttl);
        loop {
            let mut token = [0u8; TOKEN_LEN];
            rand::rngs::OsRng.fill_bytes(&mut token);
            if guard.contains_key(&token) {
                continue;
            }
            let (tx, rx) = oneshot::channel();
            guard.insert(
                token,
                PendingTransfer {
                    peer,
                    kind,
                    vpath: vpath.to_string(),
                    issued_at: Instant::now(),
                    done: tx,
                },
            );
            return (token, rx);
        }
    }

    /// 数据流入站兑付：来源节点一致且未过期才放行；对端不符不动令牌
    /// （令牌 256 位随机不可猜，留给合法节点兑付），过期则作废移除。
    pub(crate) fn take(&self, token: &[u8; TOKEN_LEN], peer: &PeerId) -> Option<PendingTransfer> {
        let mut guard = self.lock();
        let valid = guard
            .get(token)
            .is_some_and(|p| &p.peer == peer && p.issued_at.elapsed() < self.token_ttl);
        if valid {
            guard.remove(token)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(tag: u8) -> PeerId {
        PeerId::from_bytes([tag; 32])
    }

    #[test]
    fn issue_take_roundtrip_and_peer_binding() {
        let reg = TransferRegistry::new(Duration::from_secs(60));
        let (token, _rx) = reg.issue(peer(1), DataKind::Get, "/a");
        assert!(reg.take(&token, &peer(2)).is_none(), "换节点兑付必须拒绝");
        let pending = reg.take(&token, &peer(1)).unwrap();
        assert_eq!(pending.kind, DataKind::Get);
        assert_eq!(pending.vpath, "/a");
    }

    #[test]
    fn expired_tokens_are_rejected() {
        let reg = TransferRegistry::new(Duration::from_secs(0));
        let (token, _) = reg.issue(peer(1), DataKind::Put, "/a");
        assert!(reg.take(&token, &peer(1)).is_none(), "TTL=0 立即过期");
    }
}

/// 数据通道首帧载荷。
pub fn data_header(op: u8, token: &[u8; TOKEN_LEN]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(1 + TOKEN_LEN);
    frame.push(op);
    frame.extend_from_slice(token);
    frame
}

/// 解析数据通道首帧；长度不符或令牌缺字返回 None（由调用方断流）。
pub fn parse_data_header(frame: &[u8]) -> Option<(u8, [u8; TOKEN_LEN])> {
    let (&op, rest) = frame.split_first()?;
    if rest.len() != TOKEN_LEN {
        return None;
    }
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(rest);
    Some((op, token))
}

#[cfg(test)]
mod data_header_tests {
    use super::{data_header, parse_data_header, TOKEN_LEN};
    use crate::wire::DATA_OP_PUT;
    #[test]
    fn data_header_roundtrip() {
        let mut token = [1u8; TOKEN_LEN];
        token[7] = 9;
        let frame = data_header(DATA_OP_PUT, &token);
        assert_eq!(parse_data_header(&frame), Some((DATA_OP_PUT, token)));
        assert!(parse_data_header(&frame[..10]).is_none());
    }
}
