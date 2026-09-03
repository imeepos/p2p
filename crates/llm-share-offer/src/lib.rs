//! llm-share 能力声明（offer）：组装/验签/订阅簿/选路。
//!
//! 设计依据 idle-token-sharing-plan §5.2/§6/§10（MVP A5 后半）：
//! - 发布经 p2p-discovery 的 rendezvous 签名注册通道（专用 namespace 宣告在场与 TTL），
//!   对其公开设施编程，不改 crates/p2p-* 内核；
//! - 声明本体以 [`offer::SignedOffer`] 信封（canonical JSON + Ed25519）交换，离线验签，
//!   TTL 过期即失效、不再被选路；
//! - 纯应用层：发布帧组装与选路均为纯函数，便于 proxy 后续消费。

pub mod offer;
pub mod publish;
pub mod route;

pub use offer::{Offer, OfferError, RateLimit, SignedOffer, VerifyError, PROTOCOL_ID};
pub use publish::{announce_register, OfferBook, OFFER_NAMESPACE};
pub use route::{select_offers, RouteCandidate};
