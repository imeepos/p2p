//! 契约 §16.6 v13 provider/share/redeem 主流程（plain fns，tests 直调）：
//! 写路径全部经 p2p-cli llm_share 共享事实源（与 CLI 同一条 tmp+rename 原子
//! 写），redeem 借方编排复用 share_redeem::run；查询命令零写入（§16.2.4）。
//! apiKey 明文只经入参落 0600 密钥文件，禁进日志/台账/链接。

use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::provider::{self, Protocol, ProviderView, SaveParams};
use p2p_cli::llm_share::share::{
    self, ShareCreateParams, ShareCreateReport, ShareListReport, ShareRevokeReport,
};
use p2p_cli::llm_share::share_redeem::{self, RedeemOutcome, RedeemParams};

use super::inputs::{LlmProviderSaveInput, LlmShareCreateInput};
use super::share_views::{LlmProviderListView, LlmProviderRemoveView};
use super::LlmShareStore;
use crate::types::GuiConfig;

/// llm_share_provider_save：IPC 必填集校验 → 协议解析 → 保存 → 掩码视图。
pub fn provider_save(
    store: &LlmShareStore,
    input: LlmProviderSaveInput,
) -> Result<ProviderView, String> {
    let api_key = input.api_key.trim().to_owned();
    let protocol = parse_protocol(&input.protocol)?;
    let params = SaveParams {
        id: input.id,
        name: input.name,
        base_url: input.base_url,
        protocol,
        api_key,
        models: input.models,
        created_at: now_secs(),
    };
    let config = provider::save(&store.data_dir(), params)?;
    warn_insecure_base_url(&config.base_url);
    Ok(ProviderView {
        id: config.id,
        name: config.name,
        base_url: config.base_url,
        protocol: config.protocol,
        models: config.models,
        created_at: config.created_at,
        api_key_masked: provider::mask_key(input.api_key.trim()),
    })
}

/// llm_share_provider_list：apiKey 只出掩码；损坏存档显式 Err 不静默回空。
pub fn provider_list(store: &LlmShareStore) -> Result<LlmProviderListView, String> {
    Ok(LlmProviderListView {
        providers: provider::list(&store.data_dir())?,
    })
}

/// llm_share_provider_remove：不存在=显式 Err；级联删 0600 密钥文件。
pub fn provider_remove(
    store: &LlmShareStore,
    provider_id: &str,
) -> Result<LlmProviderRemoveView, String> {
    let removed = provider::remove(&store.data_dir(), provider_id)?;
    Ok(LlmProviderRemoveView {
        removed,
        provider_id: provider_id.to_owned(),
    })
}

/// llm_share_share_create：peer=本机身份；addrs=运行中节点监听地址（未运行
/// 为空表，链接无 addr 时借方走 rendezvous 查号）。
pub fn share_create(
    store: &LlmShareStore,
    cfg: &GuiConfig,
    addrs: Vec<String>,
    input: LlmShareCreateInput,
) -> Result<ShareCreateReport, String> {
    match input.max_activations {
        None | Some(1) => {}
        Some(other) => {
            return Err(format!(
                "maxActivations 固定为 1（契约 §16.6），收到 {other}"
            ));
        }
    }
    let peer = store.local_peer_id(cfg)?;
    let params = ShareCreateParams {
        peer,
        provider_id: input.provider_id,
        models: input.models,
        expires_at_unix: input.expires_at,
        note: input.note.unwrap_or_default(),
        addrs,
    };
    share::share_create(&store.data_dir(), params, now_secs())
}

/// llm_share_share_list：脱敏台账清单（status 由台账推导，永不含 token）。
pub fn share_list(store: &LlmShareStore) -> Result<ShareListReport, String> {
    share::share_list(&store.data_dir(), now_secs())
}

/// llm_share_share_revoke：置 revoked + 按 source 级联删 allowlist 条目。
pub fn share_revoke(store: &LlmShareStore, share_id: &str) -> Result<ShareRevokeReport, String> {
    share::share_revoke(&store.data_dir(), share_id)
}

/// llm_share_share_redeem：借方一次性拨号（p2p-cli 共享编排）；结构化拒绝码
/// 照契约原样透出（业务结果非 Err）。
pub async fn share_redeem(
    store: &LlmShareStore,
    cfg: &GuiConfig,
    link: String,
) -> Result<RedeemOutcome, String> {
    let params = RedeemParams {
        link,
        bootstrap: cfg.bootstrap.clone(),
        node_dir: store.node_dir(cfg).to_string_lossy().into_owned(),
        timeout_secs: share_redeem::REDEEM_TIMEOUT_SECS,
        discover_secs: share_redeem::REDEEM_DISCOVER_SECS,
    };
    share_redeem::run(&params).await
}

/// 上游协议解析：未知值显式报错（契约枚举 openai|claude）。
fn parse_protocol(raw: &str) -> Result<Protocol, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok(Protocol::OpenAI),
        "claude" => Ok(Protocol::Claude),
        other => Err(format!("protocol 非法（应为 openai|claude）：{other}")),
    }
}

/// http:// baseUrl 显式告警（契约 §16.6 #1：明文链路承载 apiKey）。
fn warn_insecure_base_url(base_url: &str) {
    if base_url.starts_with("http://") {
        tracing::warn!(
            "provider baseUrl 为 http:// 明文连接：apiKey 将经该链路明文发送（契约 §16.6 告警）"
        );
    }
}
