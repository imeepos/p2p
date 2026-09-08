//! A2A 宿主扩展（a2a-over-p2p-design §3/§5）：本地 agent 簿 + /a2a/1 handler
//! （card 相 + task 相）+ admin CRUD + rendezvous 在场发布 + task⇄ACP 桥。

pub mod admin;
pub mod agents;
pub mod bridge;
pub mod bridge_chunk;
pub mod bridge_io;
pub mod bridge_perm;
pub mod grants;
pub mod handler;
pub mod limits;
pub mod publish;
pub mod stream;
pub mod task;

#[cfg(test)]
mod agents_tests;

pub use admin::A2aAdminCtx;
pub use agents::{AgentDef, AgentStore, StoreError, AGENTS_FILE, AGENTS_MAX};
pub use grants::{GrantEntry, GrantStore, GRANTS_FILE, GRANTS_MAX};
pub use handler::{A2aDeps, A2aHandler, Subscribers};
pub use limits::{TaskGate, TASKS_TOTAL_MAX};
pub use publish::spawn_publisher;
pub use task::{TaskHandle, TaskOpError, TaskService, REGISTRY_MAX};
