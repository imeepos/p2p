//! relay 会话命令入口（E8 自 mod.rs 迁出）：降级链 2/3 跳的统一通道。

use std::io;

use p2p_identity::PeerId;
use tokio::sync::mpsc;

use super::relay_session::RelayCmd;
use super::{Mux, Swarm};

impl Swarm {
    /// 降级链 2/3 跳（打洞 + 中继电路）经由的会话命令入口。
    pub(super) async fn relay_degrade(&self, peer: PeerId) -> io::Result<Mux> {
        let senders = self.relay_sessions.lock().expect("relay lock").clone();
        if senders.is_empty() {
            tracing::debug!(%peer, "relay fallback unavailable: no relay configured");
            return Err(io::Error::other("no relay configured"));
        }
        let mut last: Option<io::Error> = None;
        for tx in senders.iter() {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if tx
                .send(RelayCmd::Degrade {
                    peer,
                    reply: reply_tx,
                })
                .await
                .is_err()
            {
                last = Some(io::Error::other("relay session closed"));
                continue;
            }
            return match reply_rx.await {
                Ok(result) => result,
                Err(_) => Err(io::Error::other("relay session dropped the request")),
            };
        }
        Err(last.unwrap_or_else(|| io::Error::other("no relay session available")))
    }

    pub(super) fn has_relay_sessions(&self) -> bool {
        !self.relay_sessions.lock().expect("relay lock").is_empty()
    }

    pub(super) fn add_relay_session(&self, tx: mpsc::Sender<RelayCmd>) {
        self.relay_sessions.lock().expect("relay lock").push(tx);
    }
}
