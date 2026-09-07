//! A2A 宿主扩展（a2a-over-p2p-design §3/§5.1/§7）：本地 agent 簿 + /a2a/1
//! card 相 handler + admin CRUD + rendezvous 在场发布。task 相在 A2A2b 落地。

pub mod admin;
pub mod agents;
pub mod handler;
pub mod publish;

#[cfg(test)]
mod agents_tests;

pub use admin::A2aAdminCtx;
pub use agents::{AgentDef, AgentStore, StoreError, AGENTS_FILE, AGENTS_MAX};
pub use handler::{A2aCardHandler, A2aDeps, Subscribers};
pub use publish::spawn_publisher;
