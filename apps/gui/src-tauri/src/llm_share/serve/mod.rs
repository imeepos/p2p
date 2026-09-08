//! 出借方常驻 serve 装配（llm-share-link 设计 §5.3，W3）：状态契约与槽位。
//! serve 与节点同生命周期：node_start（chat install 之后）装配——offer.json
//! 有效且 provider 可匹配时注册 /llm-share/proxy/1 与 /llm-share/redeem/1
//! handler（均 handle_inbound，身份取握手认证 PeerId）；node_stop 卸载。
//! 装配失败=assembled:false + lastError 落槽（可查询、不阻断节点启动，
//! 契约 §16.6：assembled:false 是常态非故障）。装配输入=启动快照，运行中
//! offer/providers 变更不热更。

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

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
    /// 未装配（节点未运行或装配失败无歧义态）缺省视图。
    pub fn not_assembled(last_error: Option<String>) -> Self {
        Self {
            assembled: false,
            provider_id: None,
            models: Vec::new(),
            last_error,
        }
    }
}

/// serve 槽位：AppState 持有（chat 槽位先例），状态与生命周期同节点。
#[derive(Default)]
pub struct ServeSlot {
    status: Mutex<Option<LlmServeStatus>>,
}

impl ServeSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前装配状态；槽空（节点未运行）回缺省 assembled:false。
    pub async fn status(&self) -> LlmServeStatus {
        self.status.lock().await.clone().unwrap_or_default()
    }

    /// 装配结果落槽（装配失败同样落槽，可查询不静默）。
    pub async fn set(&self, status: LlmServeStatus) {
        *self.status.lock().await = Some(status);
    }

    /// node_stop 卸载。
    pub async fn clear(&self) {
        *self.status.lock().await = None;
    }
}
