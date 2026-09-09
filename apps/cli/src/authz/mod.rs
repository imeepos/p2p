//! authz 命令域（authz-role-design §10，§12 A1）：role list/show/create/delete、
//! bind/unbind（--expires Unix 秒）、check（dry-run 判定输出 reason）。
//! 逻辑面在 p2p-cli::authz，本层只做 clap 参数映射与双形态输出
//! （默认 key=value 文本，--json 结构化）。数据在 <data-dir>/authz/。

mod bind;
mod check;
mod import_llm_share;
mod role;

use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum AuthzCommand {
    /// 角色管理：list/show/create/delete
    Role {
        #[command(subcommand)]
        command: role::RoleCommand,
    },
    /// 绑定 peer → 角色（单值 upsert；--expires Unix 秒）
    Bind(bind::BindArgs),
    /// 解绑 peer（回到默认拒绝；无绑定明确报错）
    Unbind(bind::UnbindArgs),
    /// dry-run 判定：输出 Allow / Deny(reason)，不做任何写
    Check(check::CheckArgs),
    /// 旧表迁移导入：allowlist/peers/grants → authz 绑定（§9，幂等可重跑）
    Import {
        #[command(subcommand)]
        command: import_llm_share::ImportCommand,
    },
}

pub async fn run(command: AuthzCommand) -> CliResult<()> {
    match command {
        AuthzCommand::Role { command } => role::run(command),
        AuthzCommand::Bind(args) => bind::bind_cmd(args),
        AuthzCommand::Unbind(args) => bind::unbind_cmd(args),
        AuthzCommand::Check(args) => check::check_cmd(args),
        AuthzCommand::Import { command } => import_llm_share::run(command),
    }
}

/// 逻辑层 String 错误 → CLI 运行失败（退出码 1）。
pub(crate) fn runtime_err(e: String) -> CliError {
    CliError::Runtime(e)
}
