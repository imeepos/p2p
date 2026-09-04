//! 桥协议与资源上限常量。协议 ID 与 crates/p2p-relay proto_ids::ACP 对齐；
//! 本库零底座依赖故持本地字面量，跨包一致性由集成卡机械验证。

/// 桥协议 ID（= proto_ids::ACP，wire-protocol.md §3.2）
pub const PROTOCOL_ID: &str = "/dsh-acp/1";

/// 握手帧 v 字段
pub const HANDSHAKE_VERSION: u32 = 1;
/// ready.bridge 版本字面量
pub const BRIDGE_VERSION: &str = "1";

/// 底座单帧上限：ndjson 行按此粒度分块（设计 §4.2-1）
pub const FRAME_CHUNK_LIMIT: usize = 1024 * 1024;
/// 单行护栏：行（含行尾换行）超限即断流（设计 §4.2-1）
pub const LINE_GUARD_LIMIT: usize = 16 * 1024 * 1024;
/// 续连窗口默认秒数（设计 §5，可配）
pub const REATTACH_WINDOW_DEFAULT_SECS: u64 = 90;
/// 每会话 update 环形缓存上限字节数（设计 §5）
pub const SESSION_UPDATE_CACHE_LIMIT: usize = 8 * 1024 * 1024;
/// 每连接会话上限（设计 §7 资源门禁）
pub const MAX_SESSIONS_PER_CONN: u32 = 4;
/// 每 peer 并发连接上限（设计 §7 资源门禁）
pub const MAX_CONNS_PER_PEER: u32 = 1;
