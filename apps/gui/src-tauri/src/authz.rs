//! gui-contract.md §18 authz 命令面（authz S3）：好友角色管理。
//!
//! 逻辑层复用 `p2p_cli::authz`（role_list/bind/unbind/check 与 p2pctl authz
//! 同一事实源，报告类型直接透出，camelCase 形状同源不另造）；本模块只做三件事：
//! 1. 数据目录裁决：authz 数据根 = app 数据目录（CLI --data-dir 等价物，
//!    §16 llm-share 同口径；非 GuiConfig.dataDir 节点数据目录）；
//! 2. 绑定列表契约视图（p2p_authz::Binding 存储形状 snake_case → §18.2 camelCase）；
//! 3. default_role 读写：消费持久化配置 authzDefaultRole 字段（§18.3 加法），
//!    save 前校验角色已登记（区分未登记与存储故障，不静默吞错）。
//!
//! 审计事件（authz.bound/unbound）由 p2p-cli 逻辑层经 p2p-authz audit sink
//! 落 <数据根>/audit.jsonl（P1b 同一份，禁双写）；审计不进 GUI 展示面。

use std::path::Path;

use p2p_authz::SystemClock;
use p2p_cli::authz::{bind, check, role_list, unbind, BindReport, CheckReport, RoleListReport, UnbindReport};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// §18.2 绑定条目契约视图：p2p_authz::Binding 的 camelCase 映射。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthzBindingJson {
    pub peer_id: String,
    pub role_id: String,
    /// 授予时刻（Unix 秒）。
    pub granted_at: u64,
    pub note: String,
    /// Unix 秒；无过期时字段不出现（对齐 p2p-cli 报告 skip-none 先例）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl From<p2p_authz::Binding> for AuthzBindingJson {
    fn from(b: p2p_authz::Binding) -> Self {
        Self {
            peer_id: b.peer_id,
            role_id: b.role_id,
            granted_at: b.granted_at,
            note: b.note,
            expires_at: b.expires_at,
        }
    }
}

/// authz_bindings_list 返回：当前绑定全集（peer 级单值）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthzBindingsReport {
    pub bindings: Vec<AuthzBindingJson>,
}

/// default_role get/save 返回：roleId 空串 = 已禁用自动绑。
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthzDefaultRoleReport {
    pub role_id: String,
}

fn data_dir(state: &AppState) -> String {
    state.data_dir().to_string_lossy().into_owned()
}

/// authz_role_list（§18.1）：内建四角色 + 自定义角色全量。
#[tauri::command]
pub async fn authz_role_list(state: State<'_, AppState>) -> Result<RoleListReport, String> {
    role_list(&data_dir(&state))
}

/// authz_bindings_list：当前绑定全集；读失败（损坏/版本不符）显式 Err 不静默。
#[tauri::command]
pub async fn authz_bindings_list(state: State<'_, AppState>) -> Result<AuthzBindingsReport, String> {
    let bindings = p2p_authz::store::load_bindings(Path::new(&data_dir(&state)))
        .map_err(|e| format!("authz 绑定表读失败: {e}"))?;
    Ok(AuthzBindingsReport {
        bindings: bindings.into_iter().map(AuthzBindingJson::from).collect(),
    })
}

/// authz_bind：upsert 绑定（含过期，Unix 秒）；成功落 authz.bound 审计事件
/// （逻辑层接线，P1b sink）。roleId 未登记 / peerId 非法 → 可读 Err。
#[tauri::command]
pub async fn authz_bind(
    state: State<'_, AppState>,
    peer_id: String,
    role_id: String,
    expires_at: Option<u64>,
    note: Option<String>,
) -> Result<BindReport, String> {
    bind(
        &data_dir(&state),
        &peer_id,
        &role_id,
        expires_at,
        note.as_deref(),
    )
}

/// authz_unbind：解绑（移除条目）；无绑定 → Err；成功落 authz.unbound 审计事件。
#[tauri::command]
pub async fn authz_unbind(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<UnbindReport, String> {
    unbind(&data_dir(&state), &peer_id)
}

/// authz_check：dry-run 判定（无绑定 = deny(NotBound) 默认拒绝可观测）；
/// permission 不在闭集 → Err（附可用 key 清单）。
#[tauri::command]
pub async fn authz_check(
    state: State<'_, AppState>,
    peer_id: String,
    permission: String,
) -> Result<CheckReport, String> {
    check(&data_dir(&state), &peer_id, &permission)
}

/// authz_default_role_get：读持久化配置 authzDefaultRole（缺省 friend；空串 = 禁用）。
#[tauri::command]
pub async fn authz_default_role_get(state: State<'_, AppState>) -> Result<AuthzDefaultRoleReport, String> {
    Ok(AuthzDefaultRoleReport {
        role_id: state.config_get().authz_default_role,
    })
}

/// authz_default_role_save：空串 = 禁用自动绑；非空必须为已登记角色（内建或
/// 自定义）。存在性校验区分「未登记」与「存储故障」：后者原样上浮不静默。
/// 原子写持久化配置（ConfigStore io_lock 串行），不改运行中节点。
#[tauri::command]
pub async fn authz_default_role_save(
    state: State<'_, AppState>,
    role_id: String,
) -> Result<AuthzDefaultRoleReport, String> {
    if !role_id.is_empty() {
        let dir = data_dir(&state);
        let exists = p2p_authz::Authz::new(Path::new(&dir), SystemClock)
            .show_role(&role_id);
        match exists {
            Ok(_) => {}
            Err(p2p_authz::AuthzError::RoleNotFound(_)) => {
                return Err(format!("角色未登记，无法设为默认: {role_id}"));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let mut cfg = state.config_get();
    cfg.authz_default_role = role_id.clone();
    state.config_save(cfg)?;
    Ok(AuthzDefaultRoleReport { role_id })
}
