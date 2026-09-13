//! default_role 自动绑共享实现（authz-a3-plan §1 S2 P1d / Amended A-3）：
//! CLI friend add 与 GUI 邀请/接受两入口共用同一实现（inventory 问题 4 收口）。
//! 已有绑定跳过（不覆盖既有授权，防 ally 被降级覆写）；角色不存在/存储读失败/
//! 绑定写失败降级警告不阻塞加好友（自动绑是旁路糖，执行判定另有默认拒绝防线）；
//! 绑定成功落 authz.bound 审计。default_role 配置读取留在各调用方（GUI/CLI 各自
//! 的 GuiConfig 同名字段 authzDefaultRole，缺省 friend）。

use std::path::Path;

use crate::audit::AuditEvent;
use crate::{Authz, SystemClock};

/// 自动绑结果（调用方据此追加用户可见文案与日志分级）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoBind {
    /// 配置空串：显式禁用，不做任何事。
    Disabled,
    /// peer 已有绑定（含既有角色 id），跳过。
    SkippedAlreadyBound { role_id: String },
    /// 绑定成功（落 authz.bound 审计事件）。
    Bound { role_id: String },
    /// 降级警告（角色不存在/存储读失败/绑定写失败），不阻塞加好友。
    Warned { reason: String },
}

impl AutoBind {
    /// 用户可见的追加说明（None = 无需追加文案）。
    pub fn note(&self) -> Option<String> {
        match self {
            AutoBind::Disabled => None,
            AutoBind::SkippedAlreadyBound { role_id } => {
                Some(format!("已持有绑定 {role_id}，跳过自动绑"))
            }
            AutoBind::Bound { role_id } => Some(format!("已自动绑定默认角色 {role_id}")),
            AutoBind::Warned { reason } => Some(format!("警告: 默认角色自动绑未生效（{reason}）")),
        }
    }
}

/// 加好友成功后的自动绑：空配置禁用 → 既有绑定检查 → bind（note=auto: default_role）。
/// 全程不返回 Err——失败只降级为 [AutoBind::Warned]。
pub fn auto_bind_default_role(data_dir: &Path, peer_id: &str, default_role: &str) -> AutoBind {
    if default_role.is_empty() {
        return AutoBind::Disabled;
    }
    // 既有绑定检查：读失败按警告降级（不阻塞加好友；判定面另有读失败=拒）。
    let bindings = match crate::store::load_bindings(data_dir) {
        Ok(bindings) => bindings,
        Err(e) => {
            return AutoBind::Warned {
                reason: format!("authz 绑定表读失败: {e}"),
            }
        }
    };
    if let Some(existing) = bindings.iter().find(|b| b.peer_id == peer_id) {
        return AutoBind::SkippedAlreadyBound {
            role_id: existing.role_id.clone(),
        };
    }
    // bind 内部：角色不存在显式报错 → 降级警告；成功 → 落 authz.bound 审计。
    let authz = Authz::new(data_dir, SystemClock);
    match authz.bind(peer_id, default_role, None, "auto: default_role") {
        Ok(bound) => {
            crate::audit::record(data_dir, &AuditEvent::bound(&bound));
            AutoBind::Bound {
                role_id: default_role.to_owned(),
            }
        }
        Err(e) => AutoBind::Warned {
            reason: format!("绑定角色 {default_role} 失败: {e}"),
        },
    }
}
