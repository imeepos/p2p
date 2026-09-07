//! ACP over P2P 桥两端共享的纯库（docs/design/acp-over-p2p-design.md §3）：
//! 握手帧编解码、ndjson 分块重组、错误码、策略表、路径约定。
//! 零网络、零进程逻辑：不做 IO 编排，不做子进程管理，文件存取路径由调用方注入。

#![forbid(unsafe_code)]

pub mod chunk;
pub mod consts;
pub mod descriptor;
pub mod error;
pub mod handshake;
pub mod paths;
pub mod policy;
pub mod share;

pub use chunk::{frames, LineReassembler};
pub use descriptor::{
    descriptor_path, read_descriptor, user_home_dir, write_descriptor, write_private_file,
    DescriptorError, LocalAgentDescriptor, DESCRIPTOR_FILE, DESCRIPTOR_SUBDIR, DESCRIPTOR_VERSION,
};
pub use error::ErrorCode;
pub use handshake::{parse_client_hello, parse_server_hello, ClientHello, Ready, ServerHello};
pub use paths::AcpPaths;
pub use policy::{AskRoute, PeerPolicy, PolicyStoreError, PolicyTable, Scope};
pub use share::{
    build_share_link, generate_token, rfc3339_from_unix, token_sha256, unix_now, ShareDenyKind,
    ShareEntry, ShareLedger, ShareSpec, ShareStoreError, DEFAULT_MAX_ACTIVATIONS,
    SHARE_FINGERPRINT_PREFIX,
};

#[cfg(test)]
mod chunk_tests;
#[cfg(test)]
mod descriptor_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod handshake_tests;
#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod share_tests;
