//! authz 命令域（authz-role-design §10，§12 A1/A2）：role list/show/create/delete、
//! bind/unbind（--expires Unix 秒）、check（dry-run 判定输出 reason）、
//! import（§9 每面独立迁移子命令：llm-share / a2a / acp）。
//! 逻辑面在 p2p-cli::authz，本层只做 clap 参数映射与双形态输出
//! （默认 key=value 文本，--json 结构化）。数据在 <data-dir>/authz/。

mod bind;
mod check;
mod import_a2a;
mod import_acp;
mod import_llm_share;
mod role;

use clap::{Args, Subcommand};

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
    /// 迁移导入（§9 每面独立子命令）：import llm-share / import a2a / import acp
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
}

/// §9 每面独立迁移子命令的统一入口，各面实现收敛在本模块对应文件。
#[derive(Subcommand)]
pub enum ImportCommand {
    /// a2a-grants.json → 有 grant 的 peer 绑 operator（幂等可重跑）
    A2a(import_a2a::ImportA2aArgs),
    /// ACP 策略表（acp-policy.json）：sandbox→guest、workspace→operator、Owner 跳过
    Acp(import_acp::AcpArgs),
    /// llm-share/allowlist.json 借方条目 → 绑定内建角色 ally（幂等可重跑）
    LlmShare(import_llm_share::LlmShareArgs),
}

pub async fn run(command: AuthzCommand) -> CliResult<()> {
    match command {
        AuthzCommand::Role { command } => role::run(command),
        AuthzCommand::Bind(args) => bind::bind_cmd(args),
        AuthzCommand::Unbind(args) => bind::unbind_cmd(args),
        AuthzCommand::Check(args) => check::check_cmd(args),
        AuthzCommand::Import { command } => match command {
            ImportCommand::A2a(args) => import_a2a::run(args),
            ImportCommand::Acp(args) => import_acp::run(args),
            ImportCommand::LlmShare(args) => {
                import_llm_share::run(import_llm_share::ImportCommand::LlmShare(args))
            }
        },
    }
}

/// 逻辑层 String 错误 → CLI 运行失败（退出码 1）。
pub(crate) fn runtime_err(e: String) -> CliError {
    CliError::Runtime(e)
}
