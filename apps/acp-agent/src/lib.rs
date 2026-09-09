//! acp-agent 库面：配置、审计、门禁、子进程监督与 /dsh-acp/1 会话编排。
//! main.rs 只是装配入口；单机回环集成测试与本生态复用都走本库。

pub mod a2a;
pub mod audit;
pub mod authz;
pub(crate) mod child;
pub mod cli;
pub mod config;
#[cfg(test)]
mod config_tests;
pub(crate) mod conn;
pub mod gate;
pub mod handler;
pub mod jail;
pub mod mcp;
pub mod permission;
#[cfg(test)]
mod permission_gate_tests;
pub mod policy;
pub mod pump;
pub mod reattach;
pub(crate) mod router;
pub mod session;
pub mod share;
pub mod subprocess;
pub mod workspaces;
#[cfg(test)]
mod workspaces_tests;

pub use audit::{AuditEvent, AuditSink, CaptureAudit, TracingAudit};
pub use config::{AgentConfig, ConfigError, WorkspaceDef, DEFAULT_WORKSPACE_ID};
pub use handler::AcpHandler;
pub use session::SessionDeps;
pub use share::{LinkContext, RedeemOutcome, ShareService};
pub use workspaces::{WorkspaceStore, WorkspaceStoreError};
