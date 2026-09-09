//! 绑定与判定逻辑（§10 bind/unbind/check）：peer 校验复用 llm_share 语义；
//! check 的权限 key 解析失败给出闭集清单提示（dry-run 不做任何写）。

use p2p_authz::Permission;

use super::reports::{BindReport, CheckReport, UnbindReport};
use super::{domain_err, facade, validate_peer};

pub fn bind(
    data_dir: &str,
    peer_id: &str,
    role_id: &str,
    expires_at: Option<u64>,
    note: Option<&str>,
) -> Result<BindReport, String> {
    validate_peer(peer_id)?;
    let bound = facade(data_dir)
        .bind(peer_id, role_id, expires_at, note.unwrap_or_default())
        .map_err(domain_err)?;
    Ok(BindReport::from(bound))
}

pub fn unbind(data_dir: &str, peer_id: &str) -> Result<UnbindReport, String> {
    validate_peer(peer_id)?;
    let removed = facade(data_dir).unbind(peer_id).map_err(domain_err)?;
    Ok(UnbindReport {
        peer_id: peer_id.to_owned(),
        role_id: removed.role_id,
    })
}

pub fn check(data_dir: &str, peer_id: &str, perm_key: &str) -> Result<CheckReport, String> {
    validate_peer(peer_id)?;
    let perm = Permission::parse(perm_key).ok_or_else(|| {
        format!(
            "权限 key 不在闭集登记表: {perm_key}（可用: {}）",
            Permission::registry()
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let decision = facade(data_dir).check(peer_id, perm).map_err(domain_err)?;
    Ok(CheckReport::of(peer_id, perm, decision))
}
