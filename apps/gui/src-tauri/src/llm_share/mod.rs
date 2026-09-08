//! gui-contract.md §16 llm-share 命令面（LSG1）：crates/llm-share-* 三件套的 Tauri 薄封装。
//!
//! 逻辑层复用 `p2p_cli::llm_share`（allowlist/offer/ledger/receipt 纯逻辑与 borrow 编排
//! 均为 pub API，chat.rs 先例的 crate 门面消费模式），与 CLI PR 轨共享同一语义事实源；
//! 本模块只做三件事：
//! 1. 契约 §16.1 serde 视图（LlmOfferView 五态 / LlmBorrowReport 顶层三态，命名 camelCase）；
//! 2. 数据目录裁决：llm-share 域文件根 = GUI app 数据目录（CLI --data-dir 等价物），
//!    节点身份种子 = GuiConfig.dataDir/key.seed；
//! 3. IPC 层显性校验（§16.1 表单必填集、§16.2.6 maxTokens 显式上限、targetPeer 无缺省）。
//!
//! §16.2.4 口径：数据文件写入只经 llm-share 流程（allow/deny/publish/borrow，与 CLI
//! 同一条 tmp+rename 原子写路径），查询命令（list/balance/receipt verify）零写入；
//! 失败路径一律可读中文 Err 上抛，禁静默吞错。

mod commands;
mod flows;
mod flows_share;
mod inputs;
pub mod serve;
mod share_views;
mod views;

#[cfg(test)]
mod tests;

// glob re-export：连带 tauri 宏生成的隐藏 __cmd__* 项（generate_handler 按本模块
// 路径解析，console 先例即命令定义在模块根；commands.rs 仅含命令壳）。
pub use commands::*;

use std::path::{Path, PathBuf};

use p2p_identity::Keypair;

use crate::types::GuiConfig;

/// 域文件子目录（对齐 p2p-cli llm_share::DIR_NAME 的落盘布局）。
const DIR_NAME: &str = "llm-share";

/// Tauri managed 状态：llm-share 域数据根（GUI app 数据目录）。
pub struct LlmShareStore {
    root: PathBuf,
}

impl LlmShareStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { root: app_data_dir }
    }

    /// 域数据根（GUI app 数据目录，CLI --data-dir 等价物）。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// p2p-cli 逻辑层的 data_dir 字符串实参（域文件 <root>/llm-share/）。
    pub fn data_dir(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// 节点数据目录（身份 key.seed 同根）：GuiConfig.dataDir，空回落 <root>/p2p-data
    /// （对齐 CLI Paths::node_data_dir 的缺省口径）。
    pub fn node_dir(&self, cfg: &GuiConfig) -> PathBuf {
        if cfg.data_dir.trim().is_empty() {
            self.root.join("p2p-data")
        } else {
            PathBuf::from(&cfg.data_dir)
        }
    }

    /// 本机身份种子路径。
    pub fn seed_path(&self, cfg: &GuiConfig) -> PathBuf {
        self.node_dir(cfg).join("key.seed")
    }

    /// 载入本机身份（缺种子显式报错，不代生成——对齐 CLI「身份缺失退出 1」语义）。
    pub fn load_keypair(&self, cfg: &GuiConfig) -> Result<Keypair, String> {
        let path = self.seed_path(cfg);
        p2p_identity::load_seed(&path).map_err(|e| {
            format!(
                "节点身份加载失败（{}）: {e}；先初始化身份再重试",
                path.display()
            )
        })
    }

    /// 本机 PeerId 字符串（净差视图视角，§16.1 ledger balance 无参取本机）。
    pub fn local_peer_id(&self, cfg: &GuiConfig) -> Result<String, String> {
        Ok(self.load_keypair(cfg)?.peer_id().to_string())
    }

    /// 本机公钥 base58（receipt verify 缺省出借方自验同一签名根）。
    pub fn local_pubkey_base58(&self, cfg: &GuiConfig) -> Result<String, String> {
        Ok(bs58::encode(self.load_keypair(cfg)?.public()).into_string())
    }

    /// 借方单笔收据 wire 文件：reqId → <root>/llm-share/receipt-<reqId>.json。
    /// reqId 拒绝路径穿越（borrow 落盘同布局，receipt verify 直读）。
    pub fn receipt_file(&self, req_id: &str) -> Result<PathBuf, String> {
        if req_id.is_empty()
            || req_id == "."
            || req_id == ".."
            || req_id.contains('/')
            || req_id.contains('\\')
            || req_id.contains("..")
        {
            return Err(format!("reqId 非法（不得为空或含路径片段）：{req_id}"));
        }
        Ok(self
            .root
            .join(DIR_NAME)
            .join(format!("receipt-{req_id}.json")))
    }
}
