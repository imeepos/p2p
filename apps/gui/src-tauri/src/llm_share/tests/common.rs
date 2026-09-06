//! 测试共用件：临时目录桩 store、GuiConfig 装配、身份种子、签名收据、JSON map 构造。

use std::path::PathBuf;

use llm_share_ledger::{Receipt, Usage};
use p2p_identity::Keypair;

use crate::llm_share::LlmShareStore;
use crate::types::GuiConfig;

pub(crate) struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("lsg1_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn store(tag: &str) -> (TempDir, LlmShareStore) {
    let temp = TempDir::new(tag);
    let store = LlmShareStore::new(temp.0.clone());
    (temp, store)
}

pub(crate) fn cfg_for(store: &LlmShareStore) -> GuiConfig {
    GuiConfig {
        data_dir: store
            .root()
            .join("node-data")
            .to_string_lossy()
            .into_owned(),
        ..GuiConfig::default()
    }
}

pub(crate) fn peer(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

pub(crate) fn seed_identity(store: &LlmShareStore, cfg: &GuiConfig) -> Keypair {
    p2p_identity::load_or_generate_seed(&store.seed_path(cfg)).expect("seed")
}

pub(crate) fn signed_receipt(lender: &Keypair, req_id: &str) -> Receipt {
    let mut receipt = Receipt {
        v: 1,
        req_id: req_id.to_owned(),
        period: "2026-09".to_owned(),
        lender: lender.peer_id().to_string(),
        borrower: peer(9),
        model: "gpt-4o".to_owned(),
        usage: Usage {
            input: 1234,
            output: 5678,
        },
        estimated: false,
        upstream_hint: "openai".to_owned(),
        ts: 1_725_400_000,
        sig: String::new(),
    };
    receipt.sign(lender).expect("sign");
    receipt
}

pub(crate) fn json_map(pairs: &[(&str, u64)]) -> std::collections::BTreeMap<String, u64> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}
