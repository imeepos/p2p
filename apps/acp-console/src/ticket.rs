//! reattach 票据存取 API 与持久化：连接成功即落盘桥签发票据 + 目标 PeerId，
//! 断流时登记窗口起点（lost_at），status 端点按窗口判定可用性。
//! tmp+rename 原子写；损坏/版本不符显式报错不静默清空（沿 acp-common 先例）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 单条票据：桥签发的续连票据 + 本端 conn uuid + 目标 PeerId。
/// `ticket`/`lost_at_unix_ms` 为加法字段（serde default）：v1 存量文件直读兼容，
/// 存量记录无桥票据按 Missing 如实上报；文件版本保持 1。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReattachTicket {
    pub conn: Uuid,
    pub peer: String,
    pub saved_at_unix_ms: u64,
    /// 桥在 ready 帧签发的续连票据（apps/acp-agent/README.md 续连票据）。
    #[serde(default)]
    pub ticket: Option<String>,
    /// 断流时刻（unix 毫秒）= 桥侧续连窗口起点；None = 连接仍在线。
    #[serde(default)]
    pub lost_at_unix_ms: Option<u64>,
}

impl ReattachTicket {
    pub fn new(conn: Uuid, peer: &str, saved_at_unix_ms: u64, ticket: Option<String>) -> Self {
        Self {
            conn,
            peer: peer.to_string(),
            saved_at_unix_ms,
            ticket,
            lost_at_unix_ms: None,
        }
    }
}

/// status /reattach 查询结果（README 契约 reason 面的数据源）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TicketQuery {
    /// 窗口内可用，携带票据与到期时刻。
    Usable(UsableTicket),
    /// 该 peer 无票据，或存量记录无桥票据（不可携回重连）。
    Missing,
    /// 断流时刻起窗口已过。
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsableTicket {
    pub ticket: String,
    pub expires_at_unix_ms: u64,
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

    /// 断流登记：把该 peer 最新票据的续连窗口起点定为断流时刻。
    /// 无票据可登记返回 false 留 debug 痕迹（拨号失败即断，属正常路径）。
    pub fn mark_lost(&self, peer: &str, lost_at_unix_ms: u64) -> std::io::Result<bool> {
        let mut file = self.load_or_fresh()?;
        let found = file.tickets.iter_mut().find(|t| t.peer == peer);
        match found {
            Some(t) => {
                t.lost_at_unix_ms = Some(lost_at_unix_ms);
                self.write_atomic(&file)?;
                Ok(true)
            }
            None => {
                tracing::debug!(peer, "mark_lost: no ticket on disk; skip");
                Ok(false)
            }
        }
    }

    /// 票据可用性判定（status /reattach 契约核心）：
    /// 在线连接（未断流）视为可用，到期时刻 = now + 窗口；
    /// 断流后窗口内可用；过期不返回（README：不返回过期票据）。
    pub fn usable_for(
        &self,
        peer: &str,
        window: Duration,
        now_unix_ms: u64,
    ) -> std::io::Result<TicketQuery> {
        let window_ms = u64::try_from(window.as_millis()).unwrap_or(u64::MAX);
        let record = match self.latest_for(peer)? {
            None => return Ok(TicketQuery::Missing),
            Some(t) => t,
        };
        let Some(ticket) = record.ticket else {
            return Ok(TicketQuery::Missing);
        };
        let anchor = record.lost_at_unix_ms.unwrap_or(now_unix_ms);
        let expires_at = anchor.saturating_add(window_ms);
        if now_unix_ms < expires_at {
            Ok(TicketQuery::Usable(UsableTicket {
                ticket,
                expires_at_unix_ms: expires_at,
            }))
        } else {
            Ok(TicketQuery::Expired)
        }
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
