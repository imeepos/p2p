//! a2a list：列出全部 agent（本机发布的 + 远程发现的）。
use clap::Args;
use serde_json::json;

use crate::error::{CliError, CliResult};
use crate::paths::Paths;

#[derive(Args)]
pub struct ListArgs {
    /// 输出 JSON 格式
    #[arg(long)]
    pub json: bool,
    /// 数据目录（默认 ./p2p-data）
    #[arg(long)]
    pub data_dir: Option<String>,
}

pub async fn run(args: ListArgs) -> CliResult<()> {
    let paths = Paths::new(args.data_dir.as_deref().unwrap_or("./p2p-data"));
    let agents_file = paths.root.join("a2a-agents.json");
    
    if !agents_file.exists() {
        if args.json {
            println!("{}", json!({ "agents": [] }));
        } else {
            println!("agents: (none)");
        }
        return Ok(());
    }

    let bytes = std::fs::read(&agents_file)
        .map_err(|e| CliError::Runtime(format!("read agents: {e}")))?;
    let agents: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| CliError::Runtime(format!("parse agents: {e}")))?;
    
    if args.json {
        println!("{}", serde_json::to_string_pretty(&agents)
            .map_err(|e| CliError::Runtime(format!("encode: {e}")))?);
    } else {
        let agents_arr = agents.get("agents").and_then(|a| a.as_array());
        match agents_arr {
            Some(arr) if !arr.is_empty() => {
                for agent in arr {
                    let id = agent.get("agentId").and_then(|v| v.as_str()).unwrap_or("?");
                    let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let vis = agent.get("visibility").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("  {id}  {name}  ({vis})");
                }
            }
            _ => println!("agents: (none)"),
        }
    }
    Ok(())
}
