//! §16.6 v13 命令面测试（PR 轨会签映射：v13_* 测试名 ↔ §16.6 语义条目）：
//! provider 存档/掩码/唯一映射/级联删 key、share 台账与 maxActivations 固定、
//! allow source/expires_at 透传、redeem 链接参数错误显式 Err、serve_status
//! 槽位缺省 assembled:false（常态非故障）。

use std::collections::BTreeMap;

use p2p_cli::llm_share::allowlist as cli_allowlist;

use super::common::{cfg_for, peer, seed_identity, store};
use crate::llm_share::{
    flows, flows_share,
    inputs::{LlmProviderSaveInput, LlmShareCreateInput},
};

fn provider_input(name: &str, models: &[&str]) -> LlmProviderSaveInput {
    LlmProviderSaveInput {
        id: None,
        name: name.to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        protocol: "openai".to_owned(),
        api_key: "sk-test-1234567890".to_owned(),
        models: models.iter().map(|m| m.to_string()).collect(),
    }
}

fn publish_offer(store: &crate::llm_share::LlmShareStore) {
    let cfg = cfg_for(store);
    seed_identity(store, &cfg);
    let input = crate::llm_share::inputs::LlmOfferPublishInput {
        models: vec!["gpt-4o".to_owned()],
        spare: BTreeMap::from([("gpt-4o".to_owned(), 150)]),
        period_ends: "2026-09-30".to_owned(),
        max_per_req: BTreeMap::new(),
        rpm: 10,
        concurrency: 2,
        ttl_secs: 3600,
        retention: None,
    };
    flows::offer_publish(store, &cfg, &input).expect("publish");
}

fn saved_provider_id(store: &crate::llm_share::LlmShareStore) -> String {
    flows_share::provider_save(store, provider_input("主号", &["gpt-4o"]))
        .expect("provider")
        .id
}

#[test]
fn v13_provider_save_list_remove_roundtrip() {
    let (_t, store) = store("v13-provider");
    let view =
        flows_share::provider_save(&store, provider_input("主号", &["gpt-4o"])).expect("save");
    assert_eq!(view.name, "主号");
    assert_eq!(view.api_key_masked, "sk-t****7890");
    let list = flows_share::provider_list(&store).expect("list");
    assert_eq!(list.providers.len(), 1);
    let id = list.providers[0].id.clone();
    let key_file = p2p_cli::llm_share::provider::key_path(&store.data_dir(), &id);
    assert!(key_file.exists(), "0600 密钥文件落盘（§16.6 #1）");
    let removed = flows_share::provider_remove(&store, &id).expect("remove");
    assert!(removed.removed);
    assert!(
        !key_file.exists(),
        "provider_remove 级联删密钥文件（§16.6 #1）"
    );
    let err = flows_share::provider_remove(&store, &id).unwrap_err();
    assert!(err.contains("不存在"), "{err}");
}

#[test]
fn v13_provider_model_unique_mapping_conflict_rejected() {
    let (_t, store) = store("v13-conflict");
    flows_share::provider_save(&store, provider_input("A", &["gpt-4o"])).expect("A");
    let err =
        flows_share::provider_save(&store, provider_input("B", &["gpt-4o", "m2"])).unwrap_err();
    assert!(err.contains("唯一映射"), "{err}");
}

#[test]
fn v13_provider_save_required_fields_explicit() {
    let (_t, store) = store("v13-req");
    let mut bad = provider_input("x", &["gpt-4o"]);
    bad.name = "  ".to_owned();
    assert!(flows_share::provider_save(&store, bad)
        .unwrap_err()
        .contains("名称不能为空"));
    let bad = provider_input("x", &[]);
    assert!(flows_share::provider_save(&store, bad)
        .unwrap_err()
        .contains("模型不能为空"));
    let mut bad = provider_input("x", &["gpt-4o"]);
    bad.protocol = "gemini".to_owned();
    assert!(flows_share::provider_save(&store, bad)
        .unwrap_err()
        .contains("protocol 非法"));
}

#[test]
fn v13_share_create_list_revoke_lifecycle() {
    let (_t, store) = store("v13-share");
    let cfg = cfg_for(&store);
    publish_offer(&store);
    let provider_id = saved_provider_id(&store);
    let input = LlmShareCreateInput {
        provider_id,
        models: None,
        expires_at: None,
        max_activations: None,
        note: Some("给好友".into()),
    };
    let report = flows_share::share_create(&store, &cfg, Vec::new(), input).expect("create");
    assert!(
        report.link.starts_with("dsh-llm-share://v1?peer="),
        "{}",
        report.link
    );
    assert_eq!(report.models, vec!["gpt-4o".to_owned()]);
    let list = flows_share::share_list(&store).expect("list");
    assert_eq!(list.shares.len(), 1);
    assert_eq!(list.shares[0].status, "active");
    let json = serde_json::to_value(&list).unwrap_or_default();
    assert!(
        !json.to_string().contains("tokenSha"),
        "台账视图不含 token 形态字段（§16.6 #2）"
    );
    let revoked = flows_share::share_revoke(&store, &report.share_id).expect("revoke");
    assert!(revoked.revoked);
    let err = flows_share::share_revoke(&store, &report.share_id).unwrap_err();
    assert!(err.contains("已撤销"), "{err}");
}

#[test]
fn v13_share_create_bounds_activations_and_offer_membership() {
    let (_t, store) = store("v13-bound");
    let cfg = cfg_for(&store);
    publish_offer(&store);
    let provider_id = saved_provider_id(&store);
    let input = LlmShareCreateInput {
        provider_id: provider_id.clone(),
        models: None,
        expires_at: None,
        max_activations: Some(2),
        note: None,
    };
    let err = flows_share::share_create(&store, &cfg, Vec::new(), input).unwrap_err();
    assert!(err.contains("固定为 1"), "{err}");
    let input = LlmShareCreateInput {
        provider_id,
        models: Some(vec!["claude-3".to_owned()]),
        expires_at: None,
        max_activations: None,
        note: None,
    };
    let err = flows_share::share_create(&store, &cfg, Vec::new(), input).unwrap_err();
    assert!(err.contains("不在当前能力声明"), "{err}");
}

#[test]
fn v13_allow_passthrough_source_and_expires_at() {
    let (_t, store) = store("v13-allow");
    flows::allow(
        &store,
        &peer(4),
        &["gpt-4o".into()],
        None,
        Some("share:sid-1"),
        Some(1_800_000_000),
    )
    .expect("allow");
    let file = cli_allowlist::path(&store.data_dir());
    let list = cli_allowlist::load_or_empty(&file).expect("load");
    let entry = list.entries.get(&peer(4)).expect("entry");
    assert_eq!(entry.source.as_deref(), Some("share:sid-1"));
    assert_eq!(entry.expires_at, Some(1_800_000_000));
    let removed =
        cli_allowlist::remove_by_source(&store.data_dir(), "share:sid-1").expect("cascade");
    assert_eq!(removed, 1, "revoke 级联按 source 删除（§16.6 #3）");
}

#[tokio::test]
async fn v13_share_redeem_bad_link_is_parameter_error_not_code() {
    let (_t, store) = store("v13-redeem");
    let cfg = cfg_for(&store);
    let outcome =
        flows_share::share_redeem(&store, &cfg, "https://example.invalid/x".to_owned()).await;
    let err = outcome.expect_err("参数错误显式 Err");
    assert!(err.contains("scheme"), "{err}");
}

#[tokio::test]
async fn v13_serve_status_defaults_to_not_assembled() {
    let slot = crate::llm_share::serve::ServeSlot::new();
    let status = slot.status().await;
    assert!(!status.assembled, "assembled:false 是常态非故障（§16.6）");
    assert!(status.models.is_empty());
    slot.set(crate::llm_share::serve::LlmServeStatus {
        assembled: true,
        provider_id: Some("p1".into()),
        models: vec!["gpt-4o".into()],
        last_error: None,
    })
    .await;
    assert!(slot.status().await.assembled);
    slot.clear().await;
    assert!(!slot.status().await.assembled);
}
