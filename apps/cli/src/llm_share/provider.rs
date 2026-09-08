//! llm-share provider 命令面（契约 §16.6 v13）：list / save / remove。
//! 逻辑在 p2p-cli::llm_share::provider；本层只做 clap 参数映射与双形态输出。
//! apiKey 入参只经 --api-key 或 stdin，禁 argv 之外的落点（§5.3 B 段）。

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::provider::{self, ProviderView, SaveParams};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

#[derive(Subcommand)]
pub enum ProviderCommand {
    /// 列出全部 provider 配置（apiKey 只出掩码）
    List(ListArgs),
    /// 保存 provider 配置（apiKey 明文只落 0600 密钥文件）
    Save(SaveArgs),
    /// 移除 provider 配置并级联删除密钥文件
    Remove(RemoveArgs),
}

/// --protocol 取值（openai|claude，缺省 openai）。
#[derive(Clone, Copy, ValueEnum)]
pub enum ProtocolArg {
    OpenAI,
    Claude,
}

impl ProtocolArg {
    fn to_logic(self) -> provider::Protocol {
        match self {
            ProtocolArg::OpenAI => provider::Protocol::OpenAI,
            ProtocolArg::Claude => provider::Protocol::Claude,
        }
    }
}

#[derive(Args)]
pub struct ListArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（provider 存档在 <data-dir>/llm-share/providers.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct SaveArgs {
    /// provider id（缺省生成 UUID）
    #[arg(long)]
    pub id: Option<String>,
    /// provider 名称（必填）
    #[arg(long, required = true)]
    pub name: String,
    /// 上游 base URL（必填）
    #[arg(long = "base-url", required = true)]
    pub base_url: String,
    /// 上游协议（openai|claude，缺省 openai）
    #[arg(long, value_enum, default_value = "openai")]
    pub protocol: ProtocolArg,
    /// apiKey（缺省从 stdin 读一行；会出现在 shell 历史/进程列表，敏感环境改用 stdin）
    #[arg(long = "api-key")]
    pub api_key: Option<String>,
    /// 模型（可重复，至少一个）
    #[arg(long = "model")]
    pub model: Vec<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct RemoveArgs {
    /// provider id（list 输出中的 id）
    pub provider_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderListReport {
    providers: Vec<ProviderView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoveReport {
    removed: bool,
    provider_id: String,
}

pub fn run(command: ProviderCommand) -> CliResult<()> {
    match command {
        ProviderCommand::List(args) => list_cmd(args),
        ProviderCommand::Save(args) => save_cmd(args),
        ProviderCommand::Remove(args) => remove_cmd(args),
    }
}

fn list_cmd(args: ListArgs) -> CliResult<()> {
    let views = provider::list(&args.data_dir).map_err(runtime_err)?;
    let report = ProviderListReport { providers: views };
    output::emit(args.json, &report, &render_list(&report.providers))
}

fn save_cmd(args: SaveArgs) -> CliResult<()> {
    let api_key = api_key_or_stdin(args.api_key)?;
    let api_key_masked = provider::mask_key(api_key.trim());
    let params = SaveParams {
        id: args.id,
        name: args.name,
        base_url: args.base_url,
        protocol: args.protocol.to_logic(),
        api_key: Some(api_key),
        models: args.model,
        created_at: now_secs(),
    };
    let config = provider::save(&args.data_dir, params).map_err(runtime_err)?;
    let view = ProviderView {
        id: config.id,
        name: config.name,
        base_url: config.base_url,
        protocol: config.protocol,
        models: config.models,
        created_at: config.created_at,
        api_key_masked,
    };
    output::emit(args.json, &view, &render_save(&view))
}

fn remove_cmd(args: RemoveArgs) -> CliResult<()> {
    let removed = provider::remove(&args.data_dir, &args.provider_id).map_err(runtime_err)?;
    let report = RemoveReport {
        removed,
        provider_id: args.provider_id,
    };
    output::emit(args.json, &report, &render_remove(&report.provider_id))
}

/// apiKey 输入：--api-key 或 stdin 一行；显式 --api-key 时提示泄漏面。
fn api_key_or_stdin(api_key: Option<String>) -> CliResult<String> {
    match api_key {
        Some(key) => {
            eprintln!("提示: --api-key 会出现在 shell 历史/进程列表，敏感环境改用 stdin 输入");
            Ok(key)
        }
        None => {
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(|e| CliError::Runtime(format!("读取 stdin apiKey 失败: {e}")))?;
            Ok(line.trim().to_owned())
        }
    }
}

fn render_list(views: &[ProviderView]) -> String {
    if views.is_empty() {
        return "暂无 provider（用 llm-share provider save 添加）".to_owned();
    }
    let mut lines = vec![format!("共 {} 个 provider", views.len())];
    for view in views {
        lines.push(format!(
            "id={}  name={}  base_url={}  protocol={}  models={}  api_key={}  created_at={}",
            view.id,
            view.name,
            view.base_url,
            view.protocol.as_str(),
            view.models.join(","),
            view.api_key_masked,
            view.created_at
        ));
    }
    lines.join("\n")
}

fn render_save(view: &ProviderView) -> String {
    format!(
        "已保存 provider id={}\nname={}\nbase_url={}\nprotocol={}\nmodels={}\napi_key={}\ncreated_at={}",
        view.id,
        view.name,
        view.base_url,
        view.protocol.as_str(),
        view.models.join(","),
        view.api_key_masked,
        view.created_at
    )
}

fn render_remove(id: &str) -> String {
    format!("已移除 provider id={id}（密钥文件级联删除）")
}
