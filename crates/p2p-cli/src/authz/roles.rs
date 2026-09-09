//! 角色管理逻辑（§10 role list/show/create/delete）：委托 p2p-authz，
//! 本层只做默认值填充（name 缺省 = role_id，note 缺省 = 空）与报告映射。

use super::reports::{
    RoleCreateReport, RoleDeleteReport, RoleListReport, RoleShowReport, RoleView,
};
use super::{domain_err, facade};

pub fn role_list(data_dir: &str) -> Result<RoleListReport, String> {
    let roles = facade(data_dir).list_roles().map_err(domain_err)?;
    Ok(RoleListReport {
        roles: roles.iter().map(RoleView::from).collect(),
    })
}

pub fn role_show(data_dir: &str, role_id: &str) -> Result<RoleShowReport, String> {
    let role = facade(data_dir).show_role(role_id).map_err(domain_err)?;
    Ok(RoleShowReport {
        role: RoleView::from(&role),
    })
}

pub fn role_create(
    data_dir: &str,
    role_id: &str,
    name: Option<&str>,
    perm_keys: &[String],
    note: Option<&str>,
) -> Result<RoleCreateReport, String> {
    let role = facade(data_dir)
        .create_role(
            role_id,
            name.unwrap_or(role_id),
            perm_keys,
            note.unwrap_or_default(),
        )
        .map_err(domain_err)?;
    Ok(RoleCreateReport {
        role: RoleView::from(&role),
    })
}

pub fn role_delete(data_dir: &str, role_id: &str) -> Result<RoleDeleteReport, String> {
    facade(data_dir).delete_role(role_id).map_err(domain_err)?;
    Ok(RoleDeleteReport {
        role_id: role_id.to_owned(),
    })
}
