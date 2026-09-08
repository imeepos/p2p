//! a2a allow/disallow：授权/撤销 peer 访问 private agent。
use clap::Args;
use serde_json::json;

use crate::error::{CliError, CliResult};
use crate::paths::Paths;

#[derive(Args)]
pub struct AllowArgs {
    /// agent ID
    #[arg(long)]
    pub agent_id: String,
    /// 对端 PeerId（base58，32 字节）
    #[arg(long)]
    pub peer_id: String,
    /// 数据目录（默认 ./p2p-data）
    #[arg(long)]
    pub data_dir: Option<String>,
    /// 输出 JSON 格式
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DisallowArgs {
    /// agent ID
    #[arg(long)]
    pub agent_id: String,
    /// 对端 PeerId（base58，32 字节）
    #[arg(long)]
    pub peer_id: String,
    /// 数据目录（默认 ./p2p-data）
    #[arg(long)]
    pub data_dir: Option<String>,
    /// 输出 JSON 格式
    #[arg(long)]
    pub json: bool,
}

pub async fn run_allow(args: AllowArgs) -> CliResult<()> {
    let paths = Paths::new(args.data_dir.as_deref().unwrap_or("./p2p-data"));
    let grants_file = paths.root.join("a2a-grants.json");
    
    // 读取现有 grants
    let mut grants: serde_json::Value = if grants_file.exists() {
        let bytes = std::fs::read(&grants_file)
            .map_err(|e| CliError::Runtime(format!("read grants: {e}")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| CliError::Runtime(format!("parse grants: {e}")))?
    } else {
        json!({ "version": 1, "grants": [] })
    };
    
    let grants_arr = grants.get_mut("grants")
        .and_then(|a| a.as_array_mut())
        .ok_or_else(|| CliError::Runtime("invalid grants file".into()))?;
    
    // 检查是否已存在
    let exists = grants_arr.iter().any(|g| {
        g.get("agentId").and_then(|v| v.as_str()) == Some(&args.agent_id)
            && g.get("peer").and_then(|v| v.as_str()) == Some(&args.peer_id)
    });
    
    if !exists {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        grants_arr.push(json!({
            "agentId": args.agent_id,
            "peer": args.peer_id,
            "grantedAt": now,
        }));
        
        // 写入文件
        let bytes = serde_json::to_vec_pretty(&grants)
            .map_err(|e| CliError::Runtime(format!("encode: {e}")))?;
        std::fs::write(&grants_file, bytes)
            .map_err(|e| CliError::Runtime(format!("write grants: {e}")))?;
    }
    
    if args.json {
        println!("{}", json!({ "granted": { "agentId": args.agent_id, "peer": args.peer_id } }));
    } else {
        println!("granted: {} -> {}", args.agent_id, args.peer_id);
    }
    Ok(())
}

pub async fn run_disallow(args: DisallowArgs) -> CliResult<()> {
    let paths = Paths::new(args.data_dir.as_deref().unwrap_or("./p2p-data"));
    let grants_file = paths.root.join("a2a-grants.json");
    
    if !grants_file.exists() {
        return Err(CliError::Runtime("grants file not found".into()));
    }
    
    let bytes = std::fs::read(&grants_file)
        .map_err(|e| CliError::Runtime(format!("read grants: {e}")))?;
    let mut grants: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| CliError::Runtime(format!("parse grants: {e}")))?;
    
    let grants_arr = grants.get_mut("grants")
        .and_then(|a| a.as_array_mut())
        .ok_or_else(|| CliError::Runtime("invalid grants file".into()))?;
    
    let before = grants_arr.len();
    grants_arr.retain(|g| {
        !(g.get("agentId").and_then(|v| v.as_str()) == Some(&args.agent_id)
            && g.get("peer").and_then(|v| v.as_str()) == Some(&args.peer_id))
    });
    
    if grants_arr.len() == before {
        return Err(CliError::Runtime(format!(
            "grant not found: {} -> {}",
            args.agent_id, args.peer_id
        )));
    }
    
    // 写入文件
    let bytes = serde_json::to_vec_pretty(&grants)
        .map_err(|e| CliError::Runtime(format!("encode: {e}")))?;
    std::fs::write(&grants_file, bytes)
        .map_err(|e| CliError::Runtime(format!("write grants: {e}")))?;
    
    if args.json {
        println!("{}", json!({ "revoked": { "agentId": args.agent_id, "peer": args.peer_id } }));
    } else {
        println!("revoked: {} -> {}", args.agent_id, args.peer_id);
    }
    Ok(())
}
