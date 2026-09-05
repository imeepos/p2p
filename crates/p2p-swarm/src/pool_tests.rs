//! 连接池单测（自 pool.rs 迁出，为 E8 回收逻辑腾出行数预算）。

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::{BoxedStream, MuxControl};

use crate::pool::{Admission, ConnectionPool, PoolDirection};

type Mux = Arc<dyn MuxControl>;

struct StubMux;

#[async_trait::async_trait]
impl MuxControl for StubMux {
    async fn open_stream(&self) -> io::Result<BoxedStream> {
        Err(io::Error::other("stub mux"))
    }
    async fn accept_stream(&self) -> Option<BoxedStream> {
        None
    }
    fn close(&self) {}
}

fn stub() -> Mux {
    Arc::new(StubMux)
}

#[test]
fn insert_is_idempotent_per_peer() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([1; 32]);
    assert!(pool.insert(peer, stub()));
    assert!(!pool.insert(peer, stub()), "second insert must lose");
    assert!(pool.get(&peer).is_some());
}

#[test]
fn take_removes_and_returns_once() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([4; 32]);
    let mux = stub();
    assert!(pool.insert(peer, mux.clone()));
    assert!(Arc::ptr_eq(&pool.take(&peer).expect("taken"), &mux));
    assert!(pool.take(&peer).is_none(), "second take must find none");
}

#[test]
fn entry_direction_tracks_last_admitted_direction() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([11; 32]);
    pool.admit_as(peer, stub(), PoolDirection::Inbound, true);
    assert_eq!(
        pool.entry_direction(&peer),
        Some(PoolDirection::Inbound),
        "admit_as must record the connection direction"
    );
    pool.admit_as(peer, stub(), PoolDirection::Outbound, true);
    assert_eq!(pool.entry_direction(&peer), Some(PoolDirection::Outbound));
}

#[test]
fn admit_accepts_into_empty_pool() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([5; 32]);
    assert!(matches!(
        pool.admit(peer, stub(), true),
        Admission::Accepted
    ));
    assert_eq!(pool.len(), 1);
}

#[test]
fn admit_prefers_new_replaces_old() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([6; 32]);
    let old = stub();
    let new = stub();
    assert!(pool.insert(peer, old.clone()));
    match pool.admit(peer, new.clone(), true) {
        Admission::Replaced(evicted) => {
            assert!(Arc::ptr_eq(&evicted, &old), "evicted must be old mux");
        }
        other => panic!("expected Replaced, got {other:?}"),
    }
    assert!(Arc::ptr_eq(&pool.get(&peer).expect("kept"), &new));
}

#[test]
fn admit_prefers_existing_returns_rejection() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([7; 32]);
    let old = stub();
    let new = stub();
    assert!(pool.insert(peer, old.clone()));
    match pool.admit(peer, new.clone(), false) {
        Admission::RejectedExisting(dup) => {
            assert!(Arc::ptr_eq(&dup, &new), "rejected must be the new mux");
        }
        other => panic!("expected RejectedExisting, got {other:?}"),
    }
    assert!(Arc::ptr_eq(&pool.get(&peer).expect("kept"), &old));
}

#[test]
fn remove_if_same_spares_replacement() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([2; 32]);
    let first = stub();
    let second = stub();
    assert!(pool.insert(peer, first.clone()));
    assert!(
        !pool.remove_if_same(&peer, &second),
        "must not remove other mux"
    );
    assert!(pool.remove_if_same(&peer, &first));
    assert!(!pool.remove_if_same(&peer, &first), "already removed");
}

/// 与 Swarm 的 dial_peer 契约一致：拨号成功方负责入池。
async fn dial_once(
    pool: Arc<ConnectionPool>,
    peer: PeerId,
    counter: Arc<AtomicUsize>,
) -> io::Result<Mux> {
    counter.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let mux = stub();
    pool.insert(peer, mux.clone());
    Ok(mux)
}

#[tokio::test]
async fn concurrent_get_or_dial_dials_once() {
    let pool = Arc::new(ConnectionPool::new());
    let peer = PeerId::from_bytes([3; 32]);
    let counter = Arc::new(AtomicUsize::new(0));
    let (r1, r2, r3) = tokio::join!(
        pool.get_or_dial(peer, dial_once(pool.clone(), peer, counter.clone())),
        pool.get_or_dial(peer, dial_once(pool.clone(), peer, counter.clone())),
        pool.get_or_dial(peer, dial_once(pool.clone(), peer, counter.clone())),
    );
    assert!(r1.is_ok() && r2.is_ok() && r3.is_ok());
    assert_eq!(counter.load(Ordering::SeqCst), 1, "dial must merge");
}

#[test]
fn reclaim_idle_takes_only_aged_entries() {
    let pool = ConnectionPool::new();
    let fresh = PeerId::from_bytes([8; 32]);
    let stale = PeerId::from_bytes([9; 32]);
    assert!(pool.insert(fresh, stub()));
    assert!(pool.insert(stale, stub()));
    // insert 记账用真实墙钟；把 now 推到 1000s 后，两连接均已逾 120s 阈值
    let now = crate::usage::unix_now() + 1000;
    let taken = pool.reclaim_idle(Duration::from_secs(120), now);
    assert_eq!(taken.len(), 2, "both aged entries must be reclaimed");
    assert!(pool.is_empty());
}

#[test]
fn reclaim_idle_spares_in_flight_entry() {
    let pool = ConnectionPool::new();
    let peer = PeerId::from_bytes([10; 32]);
    assert!(pool.insert(peer, stub()));
    let usage = pool.usage(&peer).expect("entry usage");
    let _guard = usage.enter();
    let now = crate::usage::unix_now() + 1000;
    let taken = pool.reclaim_idle(Duration::from_secs(120), now);
    assert!(
        taken.is_empty(),
        "in-flight connection must be exempt from reclaim",
    );
    assert_eq!(pool.len(), 1);
    drop(_guard);
    assert_eq!(pool.reclaim_idle(Duration::from_secs(120), now).len(), 1);
}
