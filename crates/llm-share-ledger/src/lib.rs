//! 闲置 LLM 额度共享账本（§3/§4/§5.1/§7.1，轮 52 Q1-Q3 裁决）：纯应用层不改 p2p 内核，Ed25519 复用 p2p-identity。
#![forbid(unsafe_code)]

mod error;
mod hold;
mod ledger;
mod receipt;

pub use error::{Error, Result};
pub use hold::{FreezeRequest, HoldManager, LimitPolicy};
pub use ledger::{DisputeTracker, Entry, Ledger, ReconReport, WINDOW_ESTIMATED_SECS, WINDOW_SECS};
pub use receipt::{Receipt, Usage};
