//! chat friends update 子域（IM-T43 + PR1 --addr）：分组/昵称/备注/可拨地址补丁。
//!
//! 语义对齐 GUI chat_friend_update（契约 §12.1）：至少提供一项，空串 group = 移出
//! 分组；--addr 可重复，整组替换（格式同 add）；peer 不在簿或校验拒绝 → 退出码 1。

use clap::Args;
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use super::{context, emit, runtime_err};

#[derive(Args)]
pub struct UpdateArgs {
    /// 对端 peer id
    pub peer_id: String,
    /// 分组名（trim 后 ≤32 字符；空串 = 移出分组）
    #[arg(long)]
    group: Option<String>,
    /// 昵称（trim 后 ≤64 字符；空串回退 PeerId 缩略）
    #[arg(long)]
    nickname: Option<String>,
    /// 备注（空串 = 清空）
    #[arg(long)]
    note: Option<String>,
    /// 可拨地址（ip/u端口 或 ip/t端口；可重复，整组替换）
    #[arg(long)]
    addr: Vec<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

impl UpdateArgs {
    /// 聚合补丁（测试与 report 复用同一构造路径）。
    fn patch(&self) -> p2p_chat::FriendPatch {
        p2p_chat::FriendPatch {
            group: self.group.clone(),
            nickname: self.nickname.clone(),
            note: self.note.clone(),
            addrs: (!self.addr.is_empty()).then(|| self.addr.clone()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendUpdateReport {
    peer_id: String,
    group: Option<String>,
    nickname: String,
    note: Option<String>,
    addrs: Vec<String>,
}

pub async fn run(args: UpdateArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let patch = args.patch();
    let friend = ctx
        .chat
        .friend_update(&args.peer_id, &patch)
        .map_err(runtime_err)?;
    let report = FriendUpdateReport {
        peer_id: friend.peer_id,
        group: friend.group,
        nickname: friend.nickname,
        note: friend.note,
        addrs: friend.addrs,
    };
    let text = format!(
        "已更新好友 {}（{}）group={}",
        report.nickname,
        report.peer_id,
        report.group.as_deref().unwrap_or("-"),
    );
    emit(args.json, &report, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_report_json_shape_is_judgeable() {
        let report = FriendUpdateReport {
            peer_id: "p".into(),
            group: Some("同事".into()),
            nickname: "b".into(),
            note: None,
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["group"], serde_json::json!("同事"));
        assert_eq!(v["peerId"], serde_json::json!("p"));
    }

    #[test]
    fn patch_aggregates_all_flags() {
        let args = UpdateArgs {
            peer_id: "p".into(),
            group: Some("g".into()),
            nickname: None,
            note: Some(String::new()),
            addr: vec!["127.0.0.1/u1".into()],
            json: true,
            data_dir: String::new(),
        };
        let patch = args.patch();
        assert_eq!(patch.group.as_deref(), Some("g"));
        assert!(patch.nickname.is_none());
        assert_eq!(patch.note.as_deref(), Some(""));
        assert_eq!(patch.addrs.as_deref(), Some(["127.0.0.1/u1".to_string()].as_slice()));
        assert!(!patch.is_empty());
        let empty = UpdateArgs {
            peer_id: "p".into(),
            group: None,
            nickname: None,
            note: None,
            addr: vec![],
            json: true,
            data_dir: String::new(),
        };
        assert!(empty.patch().is_empty(), "全缺补丁仍拒绝");
    }
}
