//! 票据存取与可用性判定单元测试（原 src/ticket.rs 内嵌模块，控制行数红线外移）。
use std::time::Duration;

use uuid::Uuid;

use acp_console::ticket::{
    ReattachTicket, TicketQuery, TicketStore, UsableTicket, TICKET_FILE_NAME,
};

const WINDOW: Duration = Duration::from_secs(5);

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("acp-console-ticket-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ticket(peer: &str, conn: Uuid, at: u64) -> ReattachTicket {
    ReattachTicket::new(conn, peer, at, Some("tk-bridge".into()))
}

#[test]
fn save_then_latest_roundtrip() {
    let dir = scratch("roundtrip");
    let store = TicketStore::new(&dir);
    let t = ticket("peer-a", Uuid::new_v4(), 42);
    store.save(t.clone()).unwrap();
    assert_eq!(store.latest().unwrap(), Some(t));
    assert_eq!(
        store.latest_for("peer-a").unwrap().map(|t| t.peer),
        Some("peer-a".into())
    );
    assert_eq!(store.latest_for("peer-b").unwrap(), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_overwrites_same_peer_keeps_newest_first() {
    let dir = scratch("overwrite");
    let store = TicketStore::new(&dir);
    store.save(ticket("p", Uuid::new_v4(), 1)).unwrap();
    let new = ticket("p", Uuid::new_v4(), 2);
    store.save(new.clone()).unwrap();
    assert_eq!(store.latest().unwrap(), Some(new.clone()));
    assert_eq!(store.latest_for("p").unwrap(), Some(new));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 断流登记后窗口内可用（到期 = 断流 + 窗口），过期不返回（README 契约）。
#[test]
fn usable_within_window_after_lost_then_expired() {
    let dir = scratch("window");
    let store = TicketStore::new(&dir);
    store.save(ticket("p", Uuid::new_v4(), 100)).unwrap();
    assert!(store.mark_lost("p", 1_000).unwrap());
    let ok = store.usable_for("p", WINDOW, 2_000).unwrap();
    assert_eq!(
        ok,
        TicketQuery::Usable(UsableTicket {
            ticket: "tk-bridge".into(),
            expires_at_unix_ms: 6_000,
        }),
    );
    assert_eq!(
        store.usable_for("p", WINDOW, 6_001).unwrap(),
        TicketQuery::Expired
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 在线连接（未断流）视为可用：到期从查询时刻起算一个窗口。
#[test]
fn live_connection_usable_from_now() {
    let dir = scratch("live");
    let store = TicketStore::new(&dir);
    store.save(ticket("p", Uuid::new_v4(), 100)).unwrap();
    let ok = store.usable_for("p", WINDOW, 9_000).unwrap();
    match ok {
        TicketQuery::Usable(t) => assert_eq!(t.expires_at_unix_ms, 14_000),
        other => panic!("expected usable, got {other:?}"),
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 如实反映：无记录 Missing；v1 存量记录无桥票据也 Missing（不可携回重连）。
#[test]
fn missing_for_unknown_peer_and_legacy_record() {
    let dir = scratch("legacy");
    let store = TicketStore::new(&dir);
    assert_eq!(
        store.usable_for("p", WINDOW, 1_000).unwrap(),
        TicketQuery::Missing
    );
    let legacy = format!(
        "{{\"version\":1,\"tickets\":[{{\"conn\":\"{}\",\"peer\":\"p\",\"saved_at_unix_ms\":1}}]}}",
        Uuid::new_v4()
    );
    std::fs::write(dir.join(TICKET_FILE_NAME), legacy).unwrap();
    assert_eq!(
        store.usable_for("p", WINDOW, 1_000).unwrap(),
        TicketQuery::Missing
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn corrupted_file_is_explicit_error() {
    let dir = scratch("corrupt");
    std::fs::write(dir.join(TICKET_FILE_NAME), b"{not json").unwrap();
    let store = TicketStore::new(&dir);
    assert!(store.latest().is_err());
    assert!(store.latest_for("p").is_err());
    assert!(store.usable_for("p", WINDOW, 1_000).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
