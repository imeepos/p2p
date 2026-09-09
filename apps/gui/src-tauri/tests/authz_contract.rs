//! gui-contract.md §18.2 契约形状（authz S3，纯 serde）：GUI 本地视图类型与
//! p2p-cli 报告类型的 camelCase 逐字断言（expiresAt skip-none 先例）。

use p2p_cli::authz::{BindReport, RoleListReport, RoleView, UnbindReport};
use p2p_console::authz::{AuthzBindingJson, AuthzBindingsReport, AuthzDefaultRoleReport};
use serde_json::json;

#[test]
fn authz_binding_json_camel_case_and_skips_absent_expiry() {
    let bound = AuthzBindingJson {
        peer_id: "PeerA".into(),
        role_id: "operator".into(),
        granted_at: 1_000,
        note: "备注".into(),
        expires_at: Some(2_000),
    };
    let encoded = serde_json::to_value(&bound).expect("序列化");
    assert_eq!(
        encoded,
        json!({
            "peerId": "PeerA",
            "roleId": "operator",
            "grantedAt": 1_000,
            "note": "备注",
            "expiresAt": 2_000,
        }),
        "AuthzBindingJson 字段名须逐字对齐 §18.2"
    );
    let no_expiry = AuthzBindingJson {
        expires_at: None,
        ..bound
    };
    let encoded = serde_json::to_value(&no_expiry).expect("序列化");
    assert!(
        encoded.get("expiresAt").is_none(),
        "无过期时 expiresAt 不出现（skip-none 先例）: {encoded}"
    );
}

#[test]
fn authz_bindings_report_shape() {
    let report = AuthzBindingsReport {
        bindings: vec![AuthzBindingJson {
            peer_id: "PeerA".into(),
            role_id: "friend".into(),
            granted_at: 1,
            note: String::new(),
            expires_at: None,
        }],
    };
    let encoded = serde_json::to_value(&report).expect("序列化");
    assert_eq!(
        encoded["bindings"][0]["roleId"],
        json!("friend"),
        "顶层 bindings 数组 + camelCase 条目"
    );
}

#[test]
fn authz_default_role_report_shape() {
    let encoded = serde_json::to_value(AuthzDefaultRoleReport {
        role_id: "guest".into(),
    })
    .expect("序列化");
    assert_eq!(
        encoded,
        json!({ "roleId": "guest" }),
        "§18.1 default_role 返回形状"
    );
}

#[test]
fn p2p_cli_bind_report_shape_matches_contract() {
    let report = BindReport {
        peer_id: "PeerA".into(),
        role_id: "operator".into(),
        created: true,
        granted_at: 1_000,
        expires_at: Some(2_000),
        note: "n".into(),
    };
    let encoded = serde_json::to_value(&report).expect("序列化");
    assert_eq!(
        encoded,
        json!({
            "peerId": "PeerA",
            "roleId": "operator",
            "created": true,
            "grantedAt": 1_000,
            "expiresAt": 2_000,
            "note": "n",
        }),
        "BindReport 透出形状与 §18.2 逐字一致"
    );
}

#[test]
fn p2p_cli_role_and_unbind_report_shapes() {
    let role = RoleView {
        role_id: "ally".into(),
        name: "盟友".into(),
        permissions: vec!["llm.borrow".into(), "repair.fix".into()],
        builtin: true,
        note: String::new(),
    };
    let list = RoleListReport { roles: vec![role] };
    let encoded = serde_json::to_value(&list).expect("序列化");
    assert_eq!(encoded["roles"][0]["roleId"], json!("ally"));
    assert_eq!(encoded["roles"][0]["builtin"], json!(true));
    assert_eq!(
        encoded["roles"][0]["permissions"],
        json!(["llm.borrow", "repair.fix"]),
        "permissions 为 §4 闭集 key 字符串"
    );
    let unbound = UnbindReport {
        peer_id: "PeerA".into(),
        role_id: "ally".into(),
    };
    let encoded = serde_json::to_value(&unbound).expect("序列化");
    assert_eq!(
        encoded,
        json!({ "peerId": "PeerA", "roleId": "ally" }),
        "UnbindReport 形状"
    );
}
