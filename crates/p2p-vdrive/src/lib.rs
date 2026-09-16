//! 网络硬盘协议 over P2P 底座。
//!
//! `/vdrive/fs/1`：一请求一流的文件系统操作面（stat/list/read/write/...，
//! specs/vdrive.md）。服务端挂 [FsBackend] 后端（内置 [LocalFs] 监狱实现），
//! 客户端 [VDriveClient] 实现同一 trait，WebDAV 挂载桥（[dav] + [bridge]）
//! 因此对两端同型——OS 直接挂载桥端口即可把远端目录变成系统盘符。
//!
//! 底座只做路由（design §9）：本 crate 为纯业务层，传输加密与对端身份
//! 由底座安全握手保证；访问控制经 [server::AccessPolicy] 接缝外接。

pub mod backend;
pub mod bridge;
pub mod client;
pub mod dav;
pub mod error;
pub mod http;
pub mod localfs;
pub mod path;
pub mod reply;
pub mod server;
pub mod wire;

pub use backend::FsBackend;
pub use bridge::{MountBridge, MountConfig};
pub use client::VDriveClient;
pub use error::{ErrorKind, VDriveError};
pub use localfs::LocalFs;
pub use server::{serve, serve_with_policy, AccessPolicy, AllowAll, OpClass, VDriveServer};
pub use wire::{Entry, EntryKind, Request, StatFs, MAX_CHUNK, PROTO_FS};

#[cfg(test)]
mod tests {
    #[test]
    fn protocol_id_is_valid() {
        assert!(p2p_protocol::ProtocolId::new(crate::PROTO_FS).is_ok());
    }
}
