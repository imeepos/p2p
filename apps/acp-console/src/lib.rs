//! acp-console 库面：bin 只做装配，组件独立可测（回环集成测试直接驱动这些模块）。
//! 定位见 docs/design/acp-over-p2p-design.md §3：操作者侧伴生进程，
//! 本地 WS ⇄ P2P 流哑泵 + 节点发现 + 连接状态机。

#![forbid(unsafe_code)]

pub mod config;
pub mod conn;
pub mod dial;
pub mod discovery;
pub mod out;
pub mod pump;
pub mod state;
pub mod status;
pub mod ticket;
pub mod token;
pub mod ws;

pub use config::ConsoleConfig;
pub use state::{ConnPhase, StateSnapshot, StatusHub};
