//! a2a unpublish：下架 agent（删除）。
use clap::Args;
use serde_json::json;

use crate::error::{CliError, CliResult};
use crate::paths::Paths;

#[derive(Args)]
pub struct UnpublishArgs {
    /// agent ID
    #[arg(long)]
    pub agent_id: String,
    /// 数据目录（默认 ./p2p-data）
    #[arg(long)]
    pub data_dir: Option<String>,
    /// 输出 JSON 格式
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: UnpublishArgs) -> CliResult<()> {
    let paths = Paths::new(args.data_dir.as_deref().unwrap_or("./p2p-data"));
    let agents_file = paths.root.join("a2a-agents.json");
    
    if !agents_file.exists() {
        return Err(CliError::Runtime("agents file not found".into()));
    }
    
    let bytes = std::fs::read(&agents_file).map_err(|e| CliError::Runtime(format!("read agents: {e}")))?;
    let mut agents: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| CliError::Runtime(format!("parse agents: {e}")))?;
    
    let agents_arr = agents.get_mut("agents")
        .and_then(|a| a.as_array_mut())
        .ok_or_else(|| CliError::Runtime("invalid agents file".into()))?;
    
    let before = agents_arr.len();
    agents_arr.retain(|a| {
        a.get("agentId").and_then(|v| v.as_str()) != Some(&args.agent_id)
    });
    
    if agents_arr.len() == before {
        return Err(CliError::Runtime(format!("agent not found: {}", args.agent_id)));
    }
    
    // 写入文件
    let bytes = serde_json::to_vec_pretty(&agents).map_err(|e| CliError::Runtime(format!("encode: {e}")))?;
    std::fs::write(&agents_file, bytes).map_err(|e| CliError::Runtime(format!("write agents: {e}")))?;
    
    if args.json {
        println!("{}", json!({ "removed": args.agent_id }));
    } else {
        println!("removed: {}", args.agent_id);
    }
    Ok(())
}