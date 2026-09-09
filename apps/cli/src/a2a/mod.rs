//! a2a 命令域（A2A5）：agent 管理面 list/publish/unpublish + 授权面 allow/disallow。
//! 数据面复用 apps/acp-agent 的 admin HTTP（/a2a/agents 端点），
//! 策略文件 <data-dir>/a2a-grants.json（与 acp-agent 同一目录约定）。
//! 授权语义：默认拒绝——表无条目即拒；allow=upsert（granted_at 每次刷新），
//! deny=删条目；本卡不做交互确认（headless 管理面）。

mod allow;
mod list;
mod publish;
mod unpublish;

use clap::Subcommand;

use crate::error::CliResult;

/// a2a 域命令面：agent 管理 + 授权管理（headless 管理面）。
#[derive(Subcommand)]
pub enum A2aCommand {
    /// 列出全部 agent（本机发布的 + 远程发现的）
    List(list::ListArgs),
    /// 发布 agent（创建或更新可见性）
    Publish(publish::PublishArgs),
    /// 下架 agent（删除）
    Unpublish(unpublish::UnpublishArgs),
    /// 授权 peer 访问 private agent（upsert：条目已存在则为更新并刷新 granted_at）
    Allow(allow::AllowArgs),
    /// 撤销授权（删除条目；不存在明确报错不静默）
    Disallow(allow::DisallowArgs),
}

pub async fn run(command: A2aCommand) -> CliResult<()> {
    match command {
        A2aCommand::List(args) => list::run(args).await,
        A2aCommand::Publish(args) => publish::run(args).await,
        A2aCommand::Unpublish(args) => unpublish::run(args).await,
        A2aCommand::Allow(args) => allow::run_allow(args).await,
        A2aCommand::Disallow(args) => allow::run_disallow(args).await,
    }
}
