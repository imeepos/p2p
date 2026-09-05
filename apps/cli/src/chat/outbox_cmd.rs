//! chat outbox 子域（PR1/F3）：list（按对端列 pending/failed/delivered）与
//! flush（手动补投，逐对端回报结果）。对端不可达保持 pending 属正常闭环，
//! 命令仍退出 0——结果如实回报，真正失败（IO/参数）才非零。

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use super::{context, emit, runtime_err};

#[derive(Subcommand)]
pub enum OutboxCommand {
    /// 列出各对端行箱积压与已投计数（entries 为 pending/failed 摘要）
    List(OutboxListArgs),
    /// 手动补投行箱积压（缺省全部对端；--peer 指定单个）
    Flush(OutboxFlushArgs),
}

#[derive(Args)]
pub struct OutboxListArgs {
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct OutboxFlushArgs {
    /// 只补投该对端
    #[arg(long)]
    peer: Option<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// flush 结果包装：含全空对端时给出可判空态（供自动化断言 flushed 总数）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FlushView {
    peers: Vec<p2p_chat::OutboxFlushPeerReport>,
    flushed_total: usize,
    remaining_total: usize,
}

pub async fn run(command: OutboxCommand) -> CliResult<()> {
    match command {
        OutboxCommand::List(args) => list(args).await,
        OutboxCommand::Flush(args) => flush(args).await,
    }
}

async fn list(args: OutboxListArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let rows = ctx.chat.outbox_list().map_err(runtime_err)?;
    if rows.is_empty() {
        return emit(args.json, &rows, "行箱为空（无积压、无已投记录）");
    }
    let lines: Vec<String> = rows.iter().map(fmt_row).collect();
    let text = format!("共 {} 个对端\n{}", rows.len(), lines.join("\n"));
    emit(args.json, &rows, &text)
}

async fn flush(args: OutboxFlushArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let report = ctx
        .chat
        .outbox_flush(args.peer.as_deref())
        .await
        .map_err(runtime_err)?;
    let flushed_total: usize = report.peers.iter().map(|p| p.flushed).sum();
    let remaining_total: usize = report.peers.iter().map(|p| p.remaining).sum();
    if report.peers.is_empty() {
        return emit(
            args.json,
            &FlushView { peers: vec![], flushed_total: 0, remaining_total: 0 },
            "无积压对端，无需补投",
        );
    }
    let mut text = String::new();
    for p in &report.peers {
        let err = p
            .error
            .as_deref()
            .map(|e| format!("（异常：{e}）"))
            .unwrap_or_default();
        text.push_str(&format!(
            "peer={} 补投 {} 条 剩余 {} 条{}\n",
            p.peer_id, p.flushed, p.remaining, err
        ));
    }
    text.push_str(&format!(
        "合计补投 {flushed_total} 条，剩余 {remaining_total} 条（对端不可达时保持 pending 待自动补投）"
    ));
    let view = FlushView {
        peers: report.peers,
        flushed_total,
        remaining_total,
    };
    emit(args.json, &view, &text)
}

fn fmt_row(r: &p2p_chat::OutboxPeerReport) -> String {
    let entries = r
        .entries
        .iter()
        .map(|e| format!("{:?}[{}]", e.status, e.id))
        .collect::<Vec<_>>()
        .join(" ");
    if entries.is_empty() {
        format!("- peer={} pending=0 failed=0 delivered={}", r.peer_id, r.delivered)
    } else {
        format!(
            "- peer={} pending={} failed={} delivered={} entries=[{}]",
            r.peer_id, r.pending, r.failed, r.delivered, entries
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pending: usize, failed: usize, delivered: usize) -> p2p_chat::OutboxPeerReport {
        p2p_chat::OutboxPeerReport {
            peer_id: "p".into(),
            pending,
            failed,
            delivered,
            entries: vec![p2p_chat::OutboxEntryReport {
                id: "m1".into(),
                kind: p2p_chat::ChatKind::Text,
                ts_ms: 1,
                status: p2p_chat::ChatStatus::Pending,
            }],
        }
    }

    #[test]
    fn outbox_row_text_contains_counts_and_entries() {
        let line = fmt_row(&row(1, 0, 2));
        assert!(line.contains("peer=p"));
        assert!(line.contains("pending=1"));
        assert!(line.contains("delivered=2"));
        assert!(line.contains("m1"));
    }

    #[test]
    fn outbox_report_json_shape_is_judgeable() {
        let view = FlushView {
            peers: vec![p2p_chat::OutboxFlushPeerReport {
                peer_id: "p".into(),
                flushed: 2,
                remaining: 0,
                error: None,
            }],
            flushed_total: 2,
            remaining_total: 0,
        };
        let v = serde_json::to_value(&view).unwrap();
        assert_eq!(v["flushedTotal"], serde_json::json!(2));
        assert_eq!(v["peers"][0]["peerId"], serde_json::json!("p"));
        assert_eq!(v["peers"][0]["remaining"], serde_json::json!(0));
    }
}
