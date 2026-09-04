//! 链路级注册失败必须触发重连（RS 排障 2026-09-04）：死连接上 20s 空转、
//! 查询分支被饿死、注册出现永久间隙——本用例锚定「失败即重连」契约。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::rendezvous::link::{RendezvousConn, RendezvousError, RendezvousLink};
use futures::StreamExt;

/// 每次连接都给一条「写侧已死、读侧立即 EOF」的连接：register 必失败。
struct DeadLink {
    connects: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl RendezvousLink for DeadLink {
    async fn connect(&self) -> Result<RendezvousConn, RendezvousError> {
        self.connects.fetch_add(1, Ordering::Relaxed);
        let (write, _rx) = tokio::sync::mpsc::channel(1);
        let read = futures::stream::iter(vec![Err::<Vec<u8>, RendezvousError>(
            RendezvousError::Link("dead".into()),
        )])
        .chain(futures::stream::pending());
        Ok(RendezvousConn {
            write,
            read: Box::pin(read),
        })
    }
}

#[tokio::test]
async fn register_link_failure_triggers_reconnect() {
    let connects = Arc::new(AtomicUsize::new(0));
    let link = Arc::new(DeadLink {
        connects: connects.clone(),
    });
    let keypair = Keypair::generate();
    let config = RendezvousConfig::new("rs-reconnect", keypair, link);
    let client = Arc::new(RendezvousClient::new(config));
    let (tx, _rx) = mpsc::channel(16);
    tokio::spawn(client.run(tx));

    // 首次注册即刻失败，退避初值 500ms±20%：1.5s 内至少应重拨两次。
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let n = connects.load(Ordering::Relaxed);
    assert!(n >= 2, "expected reconnect after link failure, got {n}");
}
