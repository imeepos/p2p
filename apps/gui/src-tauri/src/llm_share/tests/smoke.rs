//! 临时目录桩数据命令冒烟（PR 轨会签映射：smoke_* 测试名 ↔ §16.2 语义条目）：
//! allow/deny 默认拒绝语义、offer 生命周期与 IPC 必填集、账本双边视图、收据验签
//! PASS/FAIL 双路径、reqId 路径穿越拒绝、borrow 必填缺省报错与 reqId 幂等。

use p2p_cli::llm_share::ledger as cli_ledger;
use p2p_identity::Keypair;

use super::common::{cfg_for, json_map, peer, seed_identity, signed_receipt, store};
use crate::llm_share::{
    flows,
    inputs::{LlmBorrowRequest, LlmLedgerFilter, LlmOfferPublishInput},
    views::*,
    LlmShareStore,
};

#[test]
fn smoke_allow_deny_default_deny_semantics() {
    let (_t, store) = store("allow");
    let empty = flows::allow_list(&store).unwrap();
    assert!(empty.entries.is_empty(), "缺失文件视为空表（默认拒绝）");
    let view = flows::allow(&store, &peer(2), &["gpt-4o".into()], Some("首批")).unwrap();
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].models, vec!["gpt-4o".to_owned()]);
    assert_eq!(view.entries[0].note, "首批");
    let err = flows::deny(&store, &peer(3)).unwrap_err();
    assert!(err.contains("allowlist 无该借方条目"), "{err}");
    let view = flows::deny(&store, &peer(2)).unwrap();
    assert!(view.entries.is_empty());
}

#[test]
fn smoke_offer_publish_show_lifecycle() {
    let (_t, store) = store("offer");
    let cfg = cfg_for(&store);
    seed_identity(&store, &cfg);
    assert!(flows::offer_show(&store)
        .unwrap_err()
        .contains("暂无能力声明"));
    let input = publish_input();
    let view = flows::offer_publish(&store, &cfg, &input).unwrap();
    assert_eq!(view.peer, seed_identity(&store, &cfg).peer_id().to_string());
    assert_eq!(view.status, None, "publish 形状无 status");
    let shown = flows::offer_show(&store).unwrap();
    assert_eq!(shown.status, Some(LlmOfferStatus::Live));
    assert!(shown.remaining_secs.unwrap_or(i64::MIN) > 0);
    assert!(flows::offer_publish(&store, &cfg, &publish_input_bad_date()).is_err());
}

#[test]
fn offer_publish_ipc_required_fields_explicit() {
    let (_t, store) = store("offer-req");
    let cfg = cfg_for(&store);
    seed_identity(&store, &cfg);
    let mut input = publish_input();
    input.models.clear();
    assert!(flows::offer_publish(&store, &cfg, &input)
        .unwrap_err()
        .contains("models 必填"));
    let mut input = publish_input();
    input.spare.clear();
    assert!(flows::offer_publish(&store, &cfg, &input)
        .unwrap_err()
        .contains("spare 必填"));
    let mut input = publish_input();
    input.spare.insert("gpt-4o".into(), 0);
    assert!(
        flows::offer_publish(&store, &cfg, &input).is_err(),
        "零闲量被 crate 拒绝"
    );
}

fn publish_input() -> LlmOfferPublishInput {
    LlmOfferPublishInput {
        models: vec!["gpt-4o".into()],
        spare: json_map(&[("gpt-4o", 1_500_000)]),
        period_ends: "2026-09-30".into(),
        max_per_req: json_map(&[("gpt-4o", 128_000)]),
        rpm: 10,
        concurrency: 2,
        ttl_secs: 3600,
        retention: None,
    }
}

fn publish_input_bad_date() -> LlmOfferPublishInput {
    LlmOfferPublishInput {
        period_ends: "2026-13-01".into(),
        ..publish_input()
    }
}

#[test]
fn smoke_ledger_list_and_balance_views() {
    let (_t, store) = store("ledger");
    let cfg = cfg_for(&store);
    let self_key = seed_identity(&store, &cfg);
    let self_peer = self_key.peer_id().to_string();
    cli_ledger::record(&store.data_dir(), &signed_receipt(&self_key, "req-1")).unwrap();
    let mut incoming = signed_receipt(&Keypair::generate(), "req-2");
    incoming.borrower = self_peer.clone();
    cli_ledger::record(&store.data_dir(), &incoming).unwrap();
    let entries = flows::ledger_list(&store, &LlmLedgerFilter::default()).unwrap();
    assert_eq!(entries.len(), 2);
    let filtered = flows::ledger_list(
        &store,
        &LlmLedgerFilter {
            lender: Some(self_peer.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(filtered.len(), 1);
    let rows = flows::ledger_balance(&store, &cfg).unwrap();
    assert_eq!(rows.len(), 2, "lender/borrower 两侧各一组");
    assert!(rows
        .iter()
        .any(|r| r.net_amount > 0 && r.direction == LlmBalanceDirection::LentOut));
    assert!(rows
        .iter()
        .any(|r| r.net_amount < 0 && r.direction == LlmBalanceDirection::Borrowed));
}

#[test]
fn smoke_receipt_verify_pass_fail_paths() {
    let (_t, store) = store("verify");
    let cfg = cfg_for(&store);
    let self_key = seed_identity(&store, &cfg);
    let receipt = signed_receipt(&self_key, "req-v");
    let file = store.receipt_file("req-v").unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, serde_json::to_string_pretty(&receipt).unwrap()).unwrap();
    let explicit = flows::receipt_verify(
        &store,
        &cfg,
        "req-v",
        Some(&bs58::encode(self_key.public()).into_string()),
    )
    .unwrap();
    assert_eq!(explicit.verdict, "PASS");
    let defaulted = flows::receipt_verify(&store, &cfg, "req-v", None).unwrap();
    assert_eq!(defaulted.verdict, "PASS", "缺省公钥=本机身份（出借方自验）");
    let mut tampered = signed_receipt(&self_key, "req-v");
    tampered.usage.output += 1;
    std::fs::write(&file, serde_json::to_string_pretty(&tampered).unwrap()).unwrap();
    let failed = flows::receipt_verify(&store, &cfg, "req-v", None).unwrap();
    assert_eq!(failed.verdict, "FAIL", "FAIL 是业务结果非命令 Err");
    assert!(failed.reason.contains("验签失败"));
    assert!(flows::receipt_verify(&store, &cfg, "req-absent", None)
        .unwrap_err()
        .contains("不存在"));
    let empty_cfg_store = store.root().join("no-identity");
    let stranger = LlmShareStore::new(empty_cfg_store);
    let err = flows::receipt_verify(&stranger, &cfg_for(&stranger), "req-v", None).unwrap_err();
    assert!(err.contains("节点身份加载失败"), "{err}");
}

#[test]
fn store_receipt_file_rejects_path_traversal() {
    let (_t, store) = store("traversal");
    for bad in ["../escape", "a/b", "..", "."] {
        assert!(store.receipt_file(bad).is_err(), "{bad} 应被拒绝");
    }
    assert!(store
        .receipt_file("0198c0de-0000-7000-8000-000000000001")
        .is_ok());
}

#[test]
fn borrow_ipc_required_fields_error_paths() {
    let base = LlmBorrowRequest {
        model: None,
        messages: Some("[{\"role\":\"user\",\"content\":\"hi\"}]".into()),
        max_tokens: Some(256),
        target_peer: Some(peer(4)),
        req_id: None,
    };
    assert!(flows::normalize_borrow_request(LlmBorrowRequest {
        target_peer: None,
        ..base.clone()
    })
    .unwrap_err()
    .contains("targetPeer 必填"));
    assert!(flows::normalize_borrow_request(LlmBorrowRequest {
        max_tokens: None,
        ..base.clone()
    })
    .unwrap_err()
    .contains("maxTokens 必填"));
    assert!(flows::normalize_borrow_request(LlmBorrowRequest {
        messages: None,
        ..base.clone()
    })
    .unwrap_err()
    .contains("messages 必填"));
    let plan = flows::normalize_borrow_request(base.clone()).unwrap();
    assert_eq!(plan.req_id.len(), 36, "reqId 缺省 IPC 层生成 UUID v4");
    let plan = flows::normalize_borrow_request(LlmBorrowRequest {
        req_id: Some("same-req-id".into()),
        ..base
    })
    .unwrap();
    assert_eq!(
        plan.req_id, "same-req-id",
        "重试复用同 reqId（§16.2.3 幂等）"
    );
}
