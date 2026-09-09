//! 出借方常驻 serve 装配（llm-share-link 设计 §5.3，W3）：与节点同生命周期，
//! node_start（chat install 之后）装配——offer.json 有效且 provider 可匹配时
//! 注册 /llm-share/proxy/1 与 /llm-share/redeem/1 handler（均 handle_inbound，
//! 身份取握手认证 PeerId）；node_stop 卸载。装配输入=启动快照，运行中
//! offer/providers 变更不热更（状态可见提示）。装配失败=assembled:false +
//! lastError 落槽 + 告警日志：可查询、不阻断节点启动（契约 §16.6：
//! assembled:false 是常态非故障）。

pub mod authz;
pub mod gate;
pub mod proxy;
pub mod redeem;
pub mod replay;

use std::collections::HashMap;
use std::sync::Arc;

use llm_share_ledger::LimitPolicy;
use llm_share_proxy::server::ModelRoute;
use llm_share_proxy::upstream::Upstream;
use llm_share_proxy::{ClaudeUpstream, HttpUpstream};
use p2p::Node;
use p2p_cli::llm_share::offer as cli_offer;
use p2p_cli::llm_share::provider::{self, Protocol, ProviderView};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

use self::gate::AllowlistGate;
use self::proxy::{ProxyGateHandler, ServeCore, ServeCoreConfig};
use self::redeem::RedeemHandler;
use crate::llm_share::LlmShareStore;
use crate::types::GuiConfig;

/// LlmServeStatus（契约 §16.6 v13 表）：lastError 供面板显式告警。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmServeStatus {
    pub assembled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl LlmServeStatus {
    /// 未装配（节点未运行或装配失败）缺省视图。
    pub fn not_assembled(last_error: Option<String>) -> Self {
        Self {
            assembled: false,
            provider_id: None,
            models: Vec::new(),
            last_error,
        }
    }
}

/// 装配产物：状态快照 + 兑换/记账活性句柄。gate=None 即装配失败态，
/// allow/deny 命令面回落纯文件写路径（语义不变）。
pub(crate) struct Assembled {
    pub(crate) status: LlmServeStatus,
    pub(crate) gate: Option<Arc<AllowlistGate>>,
}

/// serve 槽位：AppState 持有（chat 槽位先例），状态与生命周期同节点。
#[derive(Default)]
pub struct ServeSlot {
    assembled: Mutex<Option<Assembled>>,
}

impl ServeSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前装配状态；槽空（节点未运行）回缺省 assembled:false。
    pub async fn status(&self) -> LlmServeStatus {
        match self.assembled.lock().await.as_ref() {
            Some(assembled) => assembled.status.clone(),
            None => LlmServeStatus::default(),
        }
    }

    /// serve 在装配时的 gate 句柄（allow/deny 与 admit 同源）；未装配 None。
    pub(crate) async fn gate(&self) -> Option<Arc<AllowlistGate>> {
        self.assembled
            .lock()
            .await
            .as_ref()
            .and_then(|a| a.gate.clone())
    }

    /// node_stop 卸载。
    pub(crate) async fn clear(&self) {
        *self.assembled.lock().await = None;
    }

    pub(crate) async fn replace(&self, assembled: Assembled) {
        *self.assembled.lock().await = Some(assembled);
    }
}

/// node_start 装配入口（state.start 在 chat install 之后调用）：失败只落状态
/// 与告警，节点照常启动（不回滚不占槽失败）。槽位已占用时拒绝重装配并留痕
///（防静默覆盖；Node facade 未暴露已注册协议查询，以本槽位占用为判据）。
pub(crate) async fn install(slot: &ServeSlot, store: &LlmShareStore, cfg: &GuiConfig, node: &Node) {
    {
        let occupied = slot.assembled.lock().await;
        if occupied.is_some() {
            warn!("llm-share serve 已装配，拒绝重复注册（防静默覆盖）");
            return;
        }
    }
    match assemble(store, cfg, node).await {
        Ok((status, gate)) => {
            slot.replace(Assembled {
                status: status.clone(),
                gate: Some(gate),
            })
            .await;
            tracing::info!(
                models = status.models.join(","),
                "llm-share serve 已装配（启动快照，运行中 offer/providers 变更不热更）"
            );
        }
        Err(reason) => {
            warn!(reason = %reason, "llm-share serve 未装配（节点照常启动）");
            slot.replace(Assembled {
                status: LlmServeStatus::not_assembled(Some(reason)),
                gate: None,
            })
            .await;
        }
    }
}

/// 装配主流程：身份 → offer 快照（live）→ provider 匹配（唯一映射）→ 路由
///（上游 + 0600 密钥）→ allowlist 门禁 → 记账核心 → 注册双 handler。
async fn assemble(
    store: &LlmShareStore,
    cfg: &GuiConfig,
    node: &Node,
) -> Result<(LlmServeStatus, Arc<AllowlistGate>), String> {
    let data_dir = store.data_dir();
    let keypair = store.load_keypair(cfg)?;
    let shown = cli_offer::show(&data_dir, p2p_cli::llm_share::now_secs())
        .map_err(|e| format!("能力声明不可用: {e}"))?;
    if shown.status != "live" {
        return Err(format!("能力声明非 live（status={}）", shown.status));
    }
    let lenders = provider::list(&data_dir)?;
    let mapping = map_models_to_providers(&shown.offer.models, &lenders)?;
    let routes = build_routes(&data_dir, &mapping)?;
    let gate = Arc::new(AllowlistGate::load(&data_dir)?);
    let spare_floor = shown.offer.spare.values().copied().min().unwrap_or(0);
    let net_limit = LimitPolicy::default().limit(spare_floor);
    let core = Arc::new(ServeCore::new(ServeCoreConfig {
        data_dir: data_dir.clone(),
        lender_id: shown.offer.peer.clone(),
        period: shown.offer.period_ends.clone(),
        net_limit,
        max_concurrent: shown.offer.rate_limit.concurrency,
        keypair,
        routes,
        gate: gate.clone(),
    }));
    node.handle_protocol(Arc::new(ProxyGateHandler::new(core)));
    node.handle_protocol(Arc::new(RedeemHandler::new(store.clone(), gate.clone())));
    let provider_id = single_provider_id(&mapping);
    Ok((
        LlmServeStatus {
            assembled: true,
            provider_id,
            models: shown.offer.models.clone(),
            last_error: None,
        },
        gate,
    ))
}

/// 模型 → provider 唯一映射（§5.3）：offer 每模型恰匹配一个 provider，
/// 零匹配=缺上游、多匹配=声明冲突，装配即拒进 lastError。
pub(crate) fn map_models_to_providers(
    models: &[String],
    lenders: &[ProviderView],
) -> Result<Vec<(String, ProviderView)>, String> {
    let mut mapping = Vec::with_capacity(models.len());
    for model in models {
        let hits: Vec<&ProviderView> = lenders
            .iter()
            .filter(|p| p.models.iter().any(|m| m == model))
            .collect();
        match hits.len() {
            1 => mapping.push((model.clone(), hits[0].clone())),
            0 => return Err(format!("模型 {model} 无 provider 匹配（上游缺失）")),
            _ => {
                let ids = hits
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                return Err(format!("模型 {model} 被 {ids} 同时声明（唯一映射冲突）"));
            }
        }
    }
    Ok(mapping)
}

/// 全部模型同源时回该 provider id（跨多 provider 时 None，面板只示模型集）。
fn single_provider_id(mapping: &[(String, ProviderView)]) -> Option<String> {
    let first = mapping.first()?.1.id.clone();
    mapping.iter().all(|(_, p)| p.id == first).then_some(first)
}

/// 装配路由：模型 → 上游实现（openai 直发 / claude 翻译，W1 交付）+ 密钥
///（0600 密钥文件直读，仅进程内存，禁日志）。
fn build_routes(
    data_dir: &str,
    mapping: &[(String, ProviderView)],
) -> Result<HashMap<String, ModelRoute>, String> {
    let mut routes = HashMap::new();
    for (model, provider) in mapping {
        let key_file = provider::key_path(data_dir, &provider.id);
        let api_key = std::fs::read_to_string(&key_file).map_err(|e| {
            format!(
                "provider {} 密钥读取失败（{}）: {e}",
                provider.name,
                key_file.display()
            )
        })?;
        let upstream: Arc<dyn Upstream> = match provider.protocol {
            Protocol::OpenAI => {
                Arc::new(HttpUpstream::new().map_err(|e| format!("openai 上游装配失败: {e}"))?)
            }
            Protocol::Claude => {
                Arc::new(ClaudeUpstream::new().map_err(|e| format!("claude 上游装配失败: {e}"))?)
            }
        };
        routes.insert(
            model.clone(),
            ModelRoute {
                base_url: provider.base_url.clone(),
                api_key: api_key.trim().to_owned(),
                upstream,
            },
        );
    }
    Ok(routes)
}
