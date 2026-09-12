//! a2a publish：发布 agent（创建或更新可见性）。
use clap::Args;
use serde_json::json;

use crate::error::{CliError, CliResult};
use crate::paths::Paths;

#[derive(Args)]
pub struct PublishArgs {
    /// agent ID（kebab-case，仅 [a-z0-9-]，<=32 字符）
    #[arg(long)]
    pub agent_id: String,
    /// agent 名称
    #[arg(long)]
    pub name: String,
    /// agent 描述
    #[arg(long)]
    pub description: String,
    /// 可见性：public/private（默认 private）
    #[arg(long, default_value = "private")]
    pub visibility: String,
    /// 数据目录（默认 ./p2p-data）
    #[arg(long)]
    pub data_dir: Option<String>,
    /// 输出 JSON 格式
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: PublishArgs) -> CliResult<()> {
    let paths = Paths::new(args.data_dir.as_deref().unwrap_or("./p2p-data"));
    let agents_file = paths.root.join("a2a-agents.json");

    // 读取现有 agents
    let mut agents: serde_json::Value = if agents_file.exists() {
        let bytes = std::fs::read(&agents_file)
            .map_err(|e| CliError::Runtime(format!("read agents: {e}")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| CliError::Runtime(format!("parse agents: {e}")))?
    } else {
        json!({ "version": 1, "agents": [] })
    };

    let agents_arr = agents
        .get_mut("agents")
        .and_then(|a| a.as_array_mut())
        .ok_or_else(|| CliError::Runtime("invalid agents file".into()))?;

    // 检查是否已存在
    let existing_idx = agents_arr
        .iter()
        .position(|a| a.get("agentId").and_then(|v| v.as_str()) == Some(&args.agent_id));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let agent = json!({
        "agentId": args.agent_id,
        "name": args.name,
        "description": args.description,
        "skills": [],
        "visibility": args.visibility,
        "enabled": true,
        "createdAt": now,
    });

    if let Some(idx) = existing_idx {
        agents_arr[idx] = agent.clone();
    } else {
        agents_arr.push(agent.clone());
    }

    // 写入文件
    let bytes = serde_json::to_vec_pretty(&agents)
        .map_err(|e| CliError::Runtime(format!("encode: {e}")))?;
    std::fs::write(&agents_file, bytes)
        .map_err(|e| CliError::Runtime(format!("write agents: {e}")))?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&agent)
                .map_err(|e| CliError::Runtime(format!("encode: {e}")))?
        );
    } else {
        println!("published: {}", args.agent_id);
    }
    Ok(())
}
