//! ACP over P2P 桥两端共享的纯库（docs/design/acp-over-p2p-design.md §3）：
//! 握手帧编解码、ndjson 分块重组、错误码、策略表、路径约定。
//! 零网络、零进程逻辑：不做 IO 编排，不做子进程管理，文件存取路径由调用方注入。

#![forbid(unsafe_code)]

pub mod chunk;
pub mod consts;
pub mod error;
pub mod handshake;
pub mod paths;
pub mod policy;

pub use chunk::{frames, LineReassembler};
pub use error::ErrorCode;
pub use handshake::{parse_client_hello, parse_server_hello, ClientHello, Ready, ServerHello};
pub use paths::AcpPaths;
pub use policy::{AskRoute, PeerPolicy, PolicyStoreError, PolicyTable, Scope};

#[cfg(test)]
mod chunk_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod handshake_tests;
#[cfg(test)]
mod policy_tests;
