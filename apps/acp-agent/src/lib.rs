//! acp-agent 库面：配置、审计、门禁、peer 归属、子进程监督与 /dsh-acp/1 会话编排。
//! main.rs 只是装配入口；单机回环集成测试与本生态复用都走本库。

pub mod audit;
pub mod cli;
pub mod config;
pub mod gate;
pub mod handler;
pub mod peers;
pub mod policy;
pub mod pump;
pub mod session;
pub mod subprocess;

pub use audit::{AuditEvent, AuditSink, CaptureAudit, TracingAudit};
pub use config::{AgentConfig, ConfigError};
pub use handler::AcpHandler;
pub use peers::PeerBook;
pub use session::SessionDeps;
