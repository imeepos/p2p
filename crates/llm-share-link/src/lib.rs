//! llm-share-link：LLM 额度分享链接的纯逻辑 crate（零网络、零进程）。
//!
//! 设计依据 docs/design/llm-share-link-design.md §5.4（C 段，W2）：
//! - [`link`]：dsh-llm-share:// 链接解析与组装（scheme/token 形态校验，未知参数忽略）；
//! - [`token`]：128-bit CSPRNG token 生成与 sha256 摘要（台账只存摘要，原文只在创建响应一次）；
//! - [`ledger`]：分享台账（shares.json 原子写；兑换激活锁内 read-check-write 防 TOCTOU）；
//! - [`redeem`]：/llm-share/redeem/1 兑换协议帧（结构化拒绝码 kebab-case）。
//!
//! 本 crate 不直接写 allowlist.json（规避 p2p-cli 文件域），兑换落 allowlist 由装配方注入回调。

pub mod ledger;
pub mod link;
pub mod redeem;
pub mod token;

pub use ledger::{LedgerError, RedeemError, RedeemResult, ShareEntry, ShareLedger};
pub use link::{build_link, is_valid_token, parse_link, LinkError, ShareLink, SCHEME};
pub use redeem::{RedeemCode, RedeemRequest, RedeemResponse, PROTOCOL_ID};
pub use token::{generate_token, token_sha256};
