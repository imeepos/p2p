//! gui-contract.md §17 a2a 命令面：agent 管理 + 授权管理。
//!
//! 逻辑层复用 apps/acp-agent 的 admin HTTP（/a2a/agents 端点），
//! 本模块只做 Tauri 薄封装。

mod commands;

pub use commands::*;
