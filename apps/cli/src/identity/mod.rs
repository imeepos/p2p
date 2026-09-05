//! identity 命令域：init/show/query + reset。
//! init 显式创建本机节点身份（0600 落盘，幂等重入输出既有身份退出 0）；
//! show 只读查询 node|chat 双身份（不起进程、不占 identity.lock）；
//! reset 危险操作必须显式 --confirm：停运行中节点后删除 key.seed（语义同 GUI）。

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::paths::Paths;
use crate::store;

mod query;
mod reset;

#[derive(Subcommand)]
pub enum IdentityCommand {
    /// 显式创建本机节点身份（0600 落盘；已存在时输出既有身份，退出 0）
    Init(query::InitArgs),
    /// 只读查看本机身份（--domain node|chat；不启动进程、不占 identity.lock）
    Show(query::ShowArgs),
    /// 重置身份：停节点 + 删除 key.seed；必须显式 --confirm
    Reset(ResetArgs),
}

#[derive(Args)]
pub struct ResetArgs {
    /// 危险操作确认（缺失即拒绝执行）
    #[arg(long)]
    confirm: bool,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// 身份双根：node 守护与 chat 聊天身份不同根（p2pctl-ai-guide §1.3/附录A）。
#[derive(Copy, Clone, ValueEnum)]
pub(crate) enum Domain {
    Node,
    Chat,
}

impl Domain {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Domain::Node => "node",
            Domain::Chat => "chat",
        }
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub async fn run(cmd: IdentityCommand) -> CliResult<()> {
    match cmd {
        IdentityCommand::Init(a) => query::init(a),
        IdentityCommand::Show(a) => query::show(a),
        IdentityCommand::Reset(a) => reset::reset(a).await,
    }
}

/// 域身份种子路径：node 走配置 dataDir（缺省 <data-dir>/p2p-data）；chat 域装配
/// 把 --data-dir 原样作节点数据目录（chat/context.rs），种子在 <data-dir>/key.seed。
pub(crate) fn seed_path(data_dir: &str, domain: Domain) -> CliResult<PathBuf> {
    let paths = Paths::new(data_dir);
    Ok(match domain {
        Domain::Node => {
            let cfg = store::load_config(&paths);
            paths.node_data_dir(Some(&cfg.data_dir)).join("key.seed")
        }
        Domain::Chat => paths.root.join("key.seed"),
    })
}

/// 身份目录 0700 + 种子 0600：存在即加载，不存在才生成，与节点装配同语义
/// （design §6）；加载路径的权限收紧由 p2p-identity 兜底。
pub(crate) fn load_or_create(seed: &Path) -> CliResult<p2p_identity::Keypair> {
    if let Some(dir) = seed.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            CliError::Runtime(format!("创建身份目录失败（{}）: {e}", dir.display()))
        })?;
        tighten_dir_mode(dir)?;
    }
    p2p_identity::load_or_generate_seed(seed)
        .map_err(|e| CliError::Runtime(format!("身份创建失败（{}）: {e}", seed.display())))
}

#[cfg(unix)]
fn tighten_dir_mode(dir: &Path) -> CliResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(dir)
        .map_err(|e| CliError::Runtime(format!("读身份目录元数据失败（{}）: {e}", dir.display())))?
        .permissions();
    perms.set_mode(0o700);
    std::fs::set_permissions(dir, perms)
        .map_err(|e| CliError::Runtime(format!("收紧身份目录权限失败（{}）: {e}", dir.display())))
}

#[cfg(not(unix))]
fn tighten_dir_mode(_dir: &Path) -> CliResult<()> {}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("p2pctl-id-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn seed_paths_differ_by_domain_root() {
        let dir = temp_dir("roots");
        std::fs::create_dir_all(&dir).unwrap();
        let data = dir.to_string_lossy().into_owned();
        let node_seed = seed_path(&data, Domain::Node).unwrap();
        let chat_seed = seed_path(&data, Domain::Chat).unwrap();
        assert_eq!(node_seed, dir.join("p2p-data").join("key.seed"));
        assert_eq!(chat_seed, dir.join("key.seed"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
