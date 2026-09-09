//! 闲置 LLM 额度共享代理（E10-T19，idle-token-sharing-plan §4/§5/§6/§7.1）：
//! `/llm-share/proxy/1` 服务端处理器（三闸准入 + SSE 逐帧转发 + 预授权结算 + 签名收据）与拨号客户端。
//! 纯应用层：只消费 p2p-protocol / llm-share-ledger 公共 API，不改内核；
//! 上游 key 仅存出借方进程内存，落盘必须 0600（[keystore]）。
//! 双协议上游：openai 直发（[HttpUpstream]），claude 请求/SSE 双向翻译（[ClaudeUpstream]）。
#![forbid(unsafe_code)]

pub mod claude_upstream;
pub mod client;
pub mod error;
pub mod gate1;
pub mod keystore;
pub mod serve;
pub mod server;
pub mod sse;
pub mod upstream;
pub mod upstream_http;
pub mod wire;

pub use claude_upstream::ClaudeUpstream;
pub use client::{ProxyClient, ProxyEvent};
pub use error::{ErrorCode, ProxyClientError};
pub use gate1::{AuthzChecker, Gate1Fn};
pub use server::{LenderProxy, ModelRoute, ProxyConfig};
pub use sse::estimate_tokens;
pub use upstream_http::HttpUpstream;
pub use wire::{ProxyFrame, ProxyRequest, PROTOCOL_ID};
