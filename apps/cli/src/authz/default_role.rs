//! default_role 自动绑（authz-a3-plan §1 S2 P1d）：p2pctl friend add 成功后
//! 按配置 authz.default_role（camelCase authzDefaultRole，缺省 friend；空串=
//! 禁用）自动绑定。已有绑定跳过（不覆盖既有授权，防 ally 被降级覆写）；角色
//! 不存在/存储读失败降级为警告不阻塞加好友（自动绑是旁路糖，执行判定另有
//! 默认拒绝防线）；绑定成功事件由 p2p-cli authz bind 接线落审计（P1b）。

use std::path::Path;

use p2p_authz::store;

use crate::paths::Paths;
use crate::store::load_config;

use p2p_cli::authz;

/// 自动绑结果（测试与文案分叉的判别面）。
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

/// friend add 成功后的自动绑入口：读配置 → 既有绑定检查 → bind。
/// 全程不返回 Err——自动绑失败只降级为 [AutoBind::Warned]。
pub fn auto_bind_default_role(data_dir: &str, peer_id: &str) -> AutoBind {
    let cfg = load_config(&Paths::new(data_dir));
    let role_id = cfg.authz_default_role;
    if role_id.is_empty() {
        return AutoBind::Disabled;
    }
    // 既有绑定检查：读失败按警告降级（不阻塞加好友；判定面另有读失败=拒）。
    let bindings = match store::load_bindings(Path::new(data_dir)) {
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
    match authz::bind(
        data_dir,
        peer_id,
        &role_id,
        None,
        Some("auto: default_role"),
    ) {
        Ok(_) => AutoBind::Bound { role_id },
        Err(e) => AutoBind::Warned {
            reason: format!("绑定角色 {role_id} 失败: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_authz::audit::audit_path;
    use p2p_cli::authz::bind;

    fn temp_dir(tag: &str) -> String {
        let dir =
            std::env::temp_dir().join(format!("p2pctl-autobind-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.to_str().unwrap().to_owned()
    }

    fn peer(byte: u8) -> String {
        bs58::encode([byte; 32]).into_string()
    }

    /// 写入带 authzDefaultRole 的最小 GuiConfig（camelCase，其余字段取零值）。
    fn write_config(data_dir: &str, role_json: &str) {
        let paths = Paths::new(data_dir);
        paths.ensure_dir().unwrap();
        let full = format!(
            r#"{{"quicPort":0,"tcpPort":0,"enableMdns":true,"dataDir":"","bootstrap":[],"relayAddrs":[],"advertisedAddrs":[],"observationAddrs":[],"lanOnly":false,"authzDefaultRole":{role_json}}}"#
        );
        std::fs::write(paths.config(), full).unwrap();
    }

    fn audit_lines(data_dir: &str) -> usize {
        std::fs::read_to_string(audit_path(Path::new(data_dir)))
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }

    #[test]
    fn binds_default_role_after_friend_add() {
        let dir = temp_dir("bind");
        // 缺省配置（无文件）= friend。
        let out = auto_bind_default_role(&dir, &peer(1));
        assert_eq!(
            out,
            AutoBind::Bound {
                role_id: "friend".to_owned()
            }
        );
        let bindings = store::load_bindings(Path::new(&dir)).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].role_id, "friend");
        assert_eq!(bindings[0].note, "auto: default_role");
        // 审计事件已落（P1b 接线复用）。
        let audit_text = std::fs::read_to_string(audit_path(Path::new(&dir))).unwrap();
        assert!(
            audit_text.contains("authz.bound"),
            "绑定须落审计: {audit_text}"
        );
        assert!(out.note().unwrap().contains("friend"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_binding_is_skipped_not_overwritten() {
        let dir = temp_dir("skip");
        // 预置 ally 绑定（高于 friend 阶梯），自动绑不得降级覆写。
        bind(&dir, &peer(2), "ally", None, None).unwrap();
        let before = audit_lines(&dir);
        let out = auto_bind_default_role(&dir, &peer(2));
        assert_eq!(
            out,
            AutoBind::SkippedAlreadyBound {
                role_id: "ally".to_owned()
            }
        );
        let bindings = store::load_bindings(Path::new(&dir)).unwrap();
        assert_eq!(bindings[0].role_id, "ally", "既有绑定保持不变");
        assert_eq!(audit_lines(&dir), before, "跳过不产生新审计事件");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_config_value_disables_auto_bind() {
        let dir = temp_dir("disabled");
        write_config(&dir, r#""""#);
        let out = auto_bind_default_role(&dir, &peer(3));
        assert_eq!(out, AutoBind::Disabled);
        assert_eq!(out.note(), None, "显式禁用不出警告文案");
        assert!(!audit_path(Path::new(&dir)).exists(), "禁用无任何落账");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_role_degrades_to_warning() {
        let dir = temp_dir("warn");
        write_config(&dir, r#""ghost""#);
        let out = auto_bind_default_role(&dir, &peer(4));
        let AutoBind::Warned { reason } = out.clone() else {
            panic!("角色不存在必须降级为警告: {out:?}");
        };
        assert!(reason.contains("ghost"), "警告携带角色 id: {reason}");
        let bindings = store::load_bindings(Path::new(&dir)).unwrap();
        assert!(bindings.is_empty(), "降级不产生绑定");
        assert!(
            !audit_path(Path::new(&dir)).exists(),
            "降级路径不落 bound 事件"
        );
        assert!(out.note().unwrap().starts_with("警告"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 回归防线：预置损坏绑定表 → 读失败按警告降级，不阻塞加好友主流程。
    #[test]
    fn corrupted_bindings_degrade_to_warning() {
        let dir = temp_dir("corrupt");
        let authz_dir = Path::new(&dir).join("authz");
        std::fs::create_dir_all(&authz_dir).unwrap();
        std::fs::write(authz_dir.join("bindings.json"), "{ not json").unwrap();
        let out = auto_bind_default_role(&dir, &peer(5));
        assert!(
            matches!(out, AutoBind::Warned { .. }),
            "读失败必须降级为警告: {out:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
