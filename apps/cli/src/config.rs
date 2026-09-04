//! config 命令域：对齐 GUI config_get/config_save。空态输出默认值（GUI 首跑行为），
//! save 接受完整 GuiConfig JSON（位置参数或 stdin 管道），原子写盘不触碰运行中节点。

use clap::{Args, Subcommand};
use serde_json::Value;

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::store;
use crate::types::GuiConfig;

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// 读取持久化配置（无文件输出默认值）
    Get(DirArgs),
    /// 保存完整配置 JSON（参数为 "-" 或省略时读 stdin）
    Save(SaveArgs),
}

#[derive(Args)]
pub struct DirArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct SaveArgs {
    /// 完整 GuiConfig JSON（camelCase）；"-" 或省略 = 读 stdin
    config: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

pub async fn run(cmd: ConfigCommand) -> CliResult<()> {
    match cmd {
        ConfigCommand::Get(a) => get(a),
        ConfigCommand::Save(a) => save(a),
    }
}

fn get(args: DirArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let cfg = store::load_config(&paths);
    output::emit(args.json, &cfg, &render(&cfg))
}

fn save(args: SaveArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let text = match args.config.as_deref() {
        Some("-") | None => read_stdin()?,
        Some(text) => text.to_string(),
    };
    if text.trim().is_empty() {
        return Err(CliError::Runtime(
            "配置内容为空：传入 GuiConfig JSON 或经 stdin 管道".into(),
        ));
    }
    let cfg: GuiConfig = serde_json::from_str(text.trim())
        .map_err(|e| CliError::Runtime(format!("配置 JSON 解析失败: {e}")))?;
    store::save_config(&paths, &cfg)?;
    output::emit(args.json, &cfg, &render(&cfg))
}

fn read_stdin() -> CliResult<String> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| CliError::Runtime(format!("读取 stdin 失败: {e}")))?;
    Ok(buf)
}

fn render(cfg: &GuiConfig) -> String {
    let list = |v: &[String]| v.join(",");
    let opt = |v: &Option<u16>| match v {
        Some(p) => p.to_string(),
        None => "-".into(),
    };
    [
        format!("quicPort={}", cfg.quic_port),
        format!("tcpPort={}", cfg.tcp_port),
        format!("enableMdns={}", cfg.enable_mdns),
        format!("dataDir={}", cfg.data_dir),
        format!("bootstrap={}", list(&cfg.bootstrap)),
        format!("relayAddrs={}", list(&cfg.relay_addrs)),
        format!("advertisedAddrs={}", list(&cfg.advertised_addrs)),
        format!("observationPort={}", opt(&cfg.observation_port)),
        format!("observationAddrs={}", list(&cfg.observation_addrs)),
    ]
    .join("\n")
}

/// 供单测校验默认输出可反序列化回契约类型。
#[allow(dead_code)]
fn roundtrip(cfg: &GuiConfig) -> GuiConfig {
    serde_json::from_value::<GuiConfig>(serde_json::to_value(cfg).unwrap_or(Value::Null)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_render_lists_every_field() {
        let cfg = GuiConfig::default();
        let text = render(&cfg);
        for key in [
            "quicPort=",
            "tcpPort=",
            "enableMdns=true",
            "bootstrap=",
            "relayAddrs=",
        ] {
            assert!(text.contains(key), "缺 {key}: {text}");
        }
    }
}
