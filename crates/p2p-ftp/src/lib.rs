//! FTP 语义 over P2P 底座。
//!
//! 经典 FTP 的控制/数据双通道模型映射到 p2p 流：`/ftp/ctrl/1` 承载命令与
//! 应答帧（RFC 959 命令集子集 + 三位应答码），`/ftp/data/1` 承载一次性
//! 数据流。p2p 无 IP 寻址，PORT/PASV 的「第三连接」语义改由服务端签发的
//! 一次性传输令牌承接：控制应答 `150 ok token=<hex>` 发牌，数据流首帧
//! 出示令牌，登记簿校验令牌与来源节点后执行传输。
//!
//! 底座只做路由（design §9）：本 crate 为纯业务层。宿主经 [`serve`] 把
//! 控制/数据两个 handler 注册进 [p2p::Node]；客户端用 [FtpClient] 操作。

pub mod auth;
mod client;
mod localfs;
pub mod server;
mod session;
mod session_fs;
mod transfer;
mod vfs;
mod wire;

pub use auth::{Authenticator, OpenAuth, StaticAuth};
pub use client::FtpClient;
pub use localfs::LocalFs;
pub use server::{serve, serve_with_config, FtpConfig, FtpServer};
pub use transfer::DataKind;
pub use vfs::{Entry, EntryKind, FileSystem};

use p2p_protocol::{ProtocolId, ProtocolError};

/// 控制通道协议 ID：一帧一行（命令/应答），服务端先发 220 问候。
pub const PROTO_CTRL: &str = "/ftp/ctrl/1";
/// 数据通道协议 ID：首帧 = 操作码 + 32 字节一次性令牌，其后为原始字节。
pub const PROTO_DATA: &str = "/ftp/data/1";

#[derive(Debug, thiserror::Error)]
pub enum FtpError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("对端拒绝（{code}）: {text}")]
    Rejected { code: u16, text: String },
    #[error("非法应答帧: {0}")]
    BadReply(String),
    #[error("数据流异常结束")]
    DataAborted,
    #[error("节点装配: {0}")]
    Assembly(String),
}

/// 协议 ID 构造（字面量常量，失败仅可能来自未来误改常量，显式上抛）。
pub(crate) fn proto(s: &str) -> Result<ProtocolId, FtpError> {
    ProtocolId::new(s).map_err(FtpError::from)
}
