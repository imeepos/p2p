//! reattach 票据存取 API 与持久化（需求 A）：连接成功即落盘 conn uuid + 目标 PeerId，
//! 供 ACP4 续连使用；本卡只做存取，不做续连逻辑。tmp+rename 原子写；
//! 损坏/版本不符显式报错不静默清空（沿 acp-common policy 先例）。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 单条票据：一次成功连接的 conn uuid + 目标 PeerId。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReattachTicket {
    pub conn: Uuid,
    pub peer: String,
    pub saved_at_unix_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TicketFile {
    version: u32,
    tickets: Vec<ReattachTicket>,
}

const FILE_VERSION: u32 = 1;
/// 每 peer 只留最新一条；总量截断防无限增长。
const KEEP: usize = 8;

pub const TICKET_FILE_NAME: &str = "reattach-tickets.json";

/// 目录注入式存取（acp-common paths 同风格）：文件 = <root>/reattach-tickets.json。
pub struct TicketStore {
    path: PathBuf,
}

impl TicketStore {
    pub fn new(root: &Path) -> Self {
        Self {
            path: root.join(TICKET_FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 登记一次成功连接：覆盖同 peer 旧票据，置顶。
    pub fn save(&self, ticket: ReattachTicket) -> std::io::Result<()> {
        let mut file = self.load_or_fresh()?;
        file.tickets.retain(|t| t.peer != ticket.peer);
        file.tickets.insert(0, ticket);
        file.tickets.truncate(KEEP);
        self.write_atomic(&file)
    }

    /// 最新一条票据（全局最近成功连接）。
    pub fn latest(&self) -> std::io::Result<Option<ReattachTicket>> {
        Ok(self.load_or_fresh()?.tickets.into_iter().next())
    }

    /// 指定 peer 的最新票据（ACP4 续连入口）。
    pub fn latest_for(&self, peer: &str) -> std::io::Result<Option<ReattachTicket>> {
        Ok(self
            .load_or_fresh()?
            .tickets
            .into_iter()
            .find(|t| t.peer == peer))
    }

    fn load_or_fresh(&self) -> std::io::Result<TicketFile> {
        let raw = match std::fs::read(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(TicketFile {
                    version: FILE_VERSION,
                    tickets: Vec::new(),
                });
            }
            Err(e) => return Err(e),
        };
        let file: TicketFile = serde_json::from_slice(&raw).map_err(|err| {
            tracing::error!(
                error = %err,
                path = %self.path.display(),
                "reattach ticket file corrupted; refusing silent reset"
            );
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("corrupted ticket file: {err}"),
            )
        })?;
        if file.version != FILE_VERSION {
            tracing::error!(
                version = file.version,
                path = %self.path.display(),
                "reattach ticket file version unsupported"
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unsupported ticket file version {}", file.version),
            ));
        }
        Ok(file)
    }

    fn write_atomic(&self, file: &TicketFile) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        let tmp = self.path.with_extension("json.tmp");
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("acp-console-ticket-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_latest_roundtrip() {
        let dir = scratch("roundtrip");
        let store = TicketStore::new(&dir);
        let t = ReattachTicket {
            conn: Uuid::new_v4(),
            peer: "peer-a".into(),
            saved_at_unix_ms: 42,
        };
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
        let old = ReattachTicket {
            conn: Uuid::new_v4(),
            peer: "p".into(),
            saved_at_unix_ms: 1,
        };
        let new = ReattachTicket {
            conn: Uuid::new_v4(),
            peer: "p".into(),
            saved_at_unix_ms: 2,
        };
        store.save(old).unwrap();
        store.save(new.clone()).unwrap();
        assert_eq!(store.latest().unwrap(), Some(new.clone()));
        assert_eq!(store.latest_for("p").unwrap(), Some(new));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupted_file_is_explicit_error() {
        let dir = scratch("corrupt");
        std::fs::write(dir.join(TICKET_FILE_NAME), b"{not json").unwrap();
        let store = TicketStore::new(&dir);
        assert!(store.latest().is_err());
        assert!(store.latest_for("p").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
