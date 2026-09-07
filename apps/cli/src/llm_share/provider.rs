//! llm-share provider 命令面（契约 §16.6 v13）：list / save / remove。
//! 逻辑在 p2p-cli::llm_share::provider；本层只做 clap 参数映射与双形态输出。
//! apiKey 入参只经 --api-key 或 stdin，禁 argv 之外的落点（§5.3 B 段）。

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;