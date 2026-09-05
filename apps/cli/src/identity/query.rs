//! identity init/show：身份正门与只读查询。
//! init 补 F6 缺口（新目录首命令必败、只能借道起进程造身份）；
//! show 补只读缺口（此前查 chat 身份只能起 chat serve 读首行，还占锁）。

use clap::Args;
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::{load_or_create, seed_path, Domain};

#[derive(Args)]
pub struct InitArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct ShowArgs {
    /// 身份域：node=守护身份（p2p-data/key.seed）；chat=聊天身份（key.seed）
    #[arg(long, value_enum, default_value_t = Domain::Node)]
    domain: Domain,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// init 结论：文本/JSON 共用（JSON camelCase，peerId 为脚本采集键）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitReport {
    created: bool,
    peer_id: String,
    public_key: String,
    seed_path: String,
    mode: &'static str,
}

/// show 结论：只读事实，不含任何写操作字段。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowReport {
    domain: &'static str,
    peer_id: String,
    public_key: String,
    seed_path: String,
}

pub(super) fn init(args: InitArgs) -> CliResult<()> {
    let seed = seed_path(&args.data_dir, Domain::Node)?;
    let created = !seed.exists();
    let keypair = load_or_create(&seed)?;
    let report = InitReport {
        created,
        peer_id: keypair.peer_id().to_string(),
        public_key: bs58::encode(keypair.public()).into_string(),
        seed_path: seed.to_string_lossy().into_owned(),
        mode: "0600",
    };
    let head = if created {
        "身份已创建"
    } else {
        "身份已存在"
    };
    let text = format!(
        "{head} peer={}\nseed={}\npubkey={}\nmode=0600",
        report.peer_id, report.seed_path, report.public_key
    );
    output::emit(args.json, &report, &text)
}

pub(super) fn show(args: ShowArgs) -> CliResult<()> {
    let seed = seed_path(&args.data_dir, args.domain)?;
    if !seed.exists() {
        return Err(crate::error::CliError::Runtime(format!(
            "{}身份不存在（{}）：node 域先运行 p2pctl identity init；chat 域由 chat serve/send 首次运行生成",
            args.domain,
            seed.display()
        )));
    }
    let keypair = p2p_identity::load_seed(&seed).map_err(|e| {
        crate::error::CliError::Runtime(format!(
            "{}身份加载失败（{}）: {e}",
            args.domain,
            seed.display()
        ))
    })?;
    let report = ShowReport {
        domain: args.domain.as_str(),
        peer_id: keypair.peer_id().to_string(),
        public_key: bs58::encode(keypair.public()).into_string(),
        seed_path: seed.to_string_lossy().into_owned(),
    };
    let text = format!(
        "domain={}\npeer={}\npubkey={}\nseed={}",
        report.domain, report.peer_id, report.public_key, report.seed_path
    );
    output::emit(args.json, &report, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::tests::temp_dir;

    fn id_path(tag: &str) -> String {
        temp_dir(tag).to_string_lossy().into_owned()
    }

    #[cfg(unix)]
    #[test]
    fn init_creates_0600_seed_and_reentry_keeps_identity() {
        use std::os::unix::fs::PermissionsExt;
        let data = id_path("init");
        init(InitArgs {
            json: true,
            data_dir: data.clone(),
        })
        .unwrap();
        let seed = seed_path(&data, Domain::Node).unwrap();
        let mode = std::fs::metadata(&seed).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "种子文件必须 0600");
        let first = p2p_identity::load_seed(&seed)
            .unwrap()
            .peer_id()
            .to_string();
        // 幂等重入：同身份不重建（exit 0 由 Ok(()) 返回保证）
        init(InitArgs {
            json: true,
            data_dir: data.clone(),
        })
        .unwrap();
        let second = p2p_identity::load_seed(&seed)
            .unwrap()
            .peer_id()
            .to_string();
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn show_missing_identity_is_explicit_error() {
        let data = id_path("show-missing");
        let err = show(ShowArgs {
            domain: Domain::Chat,
            json: true,
            data_dir: data.clone(),
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("身份不存在"), "{err}");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn show_is_read_only_for_node_domain() {
        let dir = temp_dir("show-readonly");
        let data = dir.to_string_lossy().into_owned();
        let chat_kp = p2p_identity::Keypair::generate();
        std::fs::create_dir_all(&dir).unwrap();
        p2p_identity::save_seed(&dir.join("key.seed"), &chat_kp).unwrap();
        show(ShowArgs {
            domain: Domain::Chat,
            json: true,
            data_dir: data.clone(),
        })
        .unwrap();
        // show 只读：node 侧种子不得被顺手创建
        assert!(
            !seed_path(&data, Domain::Node).unwrap().exists(),
            "show 不得生成 node 身份"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
