//! rendezvous 跨网发现（design §7.2）：客户端周期注册/查询，服务端校验签名入库。

pub mod client;
pub mod link;
pub mod messages;
pub(crate) mod reconnect;
pub mod server;

pub use client::{RendezvousClient, RendezvousConfig};
pub use link::{RendezvousConn, RendezvousError, RendezvousLink};
pub use messages::{sign_register, verify_register};
pub use server::{serve_link, RendezvousRegistry};
