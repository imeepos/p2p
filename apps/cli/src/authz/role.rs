//! p2pctl authz role 子命令：list/show/create/delete。判定与存储语义
//! 全在 p2p-authz/p2p-cli 逻辑层，本文件只有参数映射与渲染。

use clap::{Args, Subcommand};

use p2p_cli::authz::{
    role_create, role_delete, role_list, role_show, RoleCreateReport, RoleDeleteReport,
    RoleListReport, RoleShowReport, RoleView,
};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

#[derive(Subcommand)]
pub enum RoleCommand {
    /// 列出全部角色（内建四角色在前 + 自定义按 id 序）
    List(RoleListArgs),
    /// 查看单个角色（含权限清单）
    Show(RoleShowArgs),
    /// 创建自定义角色（§4 闭集任意子集；id [a-z0-9-]{1,32}，与内建名冲突即拒）
    Create(RoleCreateArgs),
    /// 删除自定义角色（仍有绑定引用即拒，先解绑再删；内建不可删）
    Delete(RoleDeleteArgs),
}

#[derive(Args)]
pub struct RoleListArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（authz 表在 <data-dir>/authz/）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct RoleShowArgs {
    /// 角色 id（内建：friend/guest/operator/ally）
    pub role_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct RoleCreateArgs {
    /// 角色 id（[a-z0-9-]{1,32}，不得与内建四名冲突）
    pub role_id: String,
    /// 展示名（缺省 = role_id）
    #[arg(long)]
    pub name: Option<String>,
    /// 权限 key（可重复，自动去重；必须是 §4 闭集内的 key）
    #[arg(long = "perm")]
    pub perm: Vec<String>,
    /// 备注
    #[arg(long)]
    pub note: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct RoleDeleteArgs {
    /// 角色 id
    pub role_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn run(command: RoleCommand) -> CliResult<()> {
    match command {
        RoleCommand::List(args) => list_cmd(args),
        RoleCommand::Show(args) => show_cmd(args),
        RoleCommand::Create(args) => create_cmd(args),
        RoleCommand::Delete(args) => delete_cmd(args),
    }
}

pub fn list_cmd(args: RoleListArgs) -> CliResult<()> {
    let report: RoleListReport = role_list(&args.data_dir).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_list(&report.roles))
}

pub fn show_cmd(args: RoleShowArgs) -> CliResult<()> {
    let report: RoleShowReport = role_show(&args.data_dir, &args.role_id).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_role(&report.role))
}

pub fn create_cmd(args: RoleCreateArgs) -> CliResult<()> {
    let report: RoleCreateReport = role_create(
        &args.data_dir,
        &args.role_id,
        args.name.as_deref(),
        &args.perm,
        args.note.as_deref(),
    )
    .map_err(runtime_err)?;
    output::emit(
        args.json,
        &report,
        &format!("已创建角色 {}", render_role(&report.role)),
    )
}

pub fn delete_cmd(args: RoleDeleteArgs) -> CliResult<()> {
    let report: RoleDeleteReport = role_delete(&args.data_dir, &args.role_id).map_err(runtime_err)?;
    output::emit(
        args.json,
        &report,
        &format!("已删除角色 {}（其绑定须先解绑）", report.role_id),
    )
}

fn render_role(role: &RoleView) -> String {
    let kind = if role.builtin { "内建" } else { "自定义" };
    format!(
        "{}, {} {}, permissions={}, note={}",
        role.role_id,
        kind,
        role.name,
        join_perms(&role.permissions),
        role.note
    )
}

fn render_list(roles: &[RoleView]) -> String {
    if roles.is_empty() {
        return "无角色（默认拒绝：无角色即无权限）".to_owned();
    }
    let builtin = roles.iter().filter(|r| r.builtin).count();
    let mut lines = vec![format!(
        "共 {} 个角色（内建 {} + 自定义 {}）",
        roles.len(),
        builtin,
        roles.len() - builtin
    )];
    lines.extend(roles.iter().map(render_role));
    lines.join("\n")
}

fn join_perms(perms: &[String]) -> String {
    if perms.is_empty() {
        "无（空角色）".to_owned()
    } else {
        perms.join(",")
    }
}
