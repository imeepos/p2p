//! registry 一致性测试（service-registry-design §2 锚定）：id 闭集逐字 /
//! 三型与默认值表逐字 / 双读规则 / serde 闭集拒读 / upsert 归一。

use crate::registry::{ServiceEntry, ServiceId, ServiceKind, ServiceRegistry};

/// §2 逐字锚点：改表即测试红（顺序敏感）。
const DESIGN_IDS: [&str; 10] = [
    "serve.llm_share",
    "serve.tunnel",
    "serve.a2a",
    "serve.acp",
    "net.rendezvous_register",
    "net.relay",
    "net.observe",
    "serve.rendezvous_server",
    "discovery.mdns",
    "net.lan_only",
];

#[test]
fn registry_is_ten_ids_verbatim() {
    assert_eq!(ServiceId::registry().len(), 10);
    let ids: Vec<_> = ServiceId::registry().iter().map(|s| s.as_str()).collect();
    assert_eq!(ids, DESIGN_IDS);
}

#[test]
fn parse_covers_registry_and_rejects_outside_ids() {
    for id in DESIGN_IDS {
        let parsed = ServiceId::parse(id).unwrap_or_else(|| panic!("表内 id 解析失败: {id}"));
        assert_eq!(parsed.as_str(), id);
    }
    assert_eq!(ServiceId::parse("serve.acp2"), None);
    assert_eq!(ServiceId::parse(""), None);
    assert_eq!(ServiceId::parse("serve.llm-share"), None);
    assert_eq!(ServiceId::parse("Serve.Acp"), None);
}

/// §2「型 / 默认值」两列逐字锚点。
#[test]
fn kinds_and_defaults_match_design_table() {
    let expected: [(&str, ServiceKind, bool); 10] = [
        ("serve.llm_share", ServiceKind::Boolean, false),
        ("serve.tunnel", ServiceKind::Boolean, false),
        ("serve.a2a", ServiceKind::Explicit, true),
        ("serve.acp", ServiceKind::Explicit, true),
        ("net.rendezvous_register", ServiceKind::Explicit, true),
        ("net.relay", ServiceKind::Explicit, true),
        ("net.observe", ServiceKind::Explicit, true),
        ("serve.rendezvous_server", ServiceKind::Explicit, true),
        ("discovery.mdns", ServiceKind::Adopted, true),
        ("net.lan_only", ServiceKind::Adopted, false),
    ];
    for (id, kind, default) in expected {
        let parsed = ServiceId::parse(id).unwrap_or_else(|| panic!("缺 id: {id}"));
        assert_eq!(parsed.kind(), kind, "kind 漂移: {id}");
        assert_eq!(parsed.kind().as_str(), kind.as_str());
        assert_eq!(parsed.default_enabled(), default, "默认值漂移: {id}");
    }
}

/// §2 双读规则锚点：收编型条目优先、缺失回落 legacy、再缺失默认；其余型无 legacy。
#[test]
fn resolve_follows_dual_read_rules() {
    let mdns = ServiceId::DISCOVERY_MDNS;
    assert!(
        !mdns.resolve(Some(false), Some(true)),
        "文件条目优先于 legacy"
    );
    assert!(!mdns.resolve(Some(false), None));
    assert!(!mdns.resolve(None, Some(false)));
    assert!(mdns.resolve(None, Some(true)), "无条目回落 legacy");
    assert!(
        mdns.resolve(None, None),
        "无条目无 legacy 回落现状默认 true"
    );
    let lan_only = ServiceId::NET_LAN_ONLY;
    assert!(!lan_only.resolve(None, None));
    assert!(lan_only.resolve(Some(true), None));
    let llm = ServiceId::SERVE_LLM_SHARE;
    assert!(!llm.resolve(None, Some(true)), "布尔型不消费 legacy 位");
    assert!(llm.resolve(Some(true), None));
    let a2a = ServiceId::SERVE_A2A;
    assert!(a2a.resolve(None, None), "显式化型默认 on 不改行为");
}

#[test]
fn serde_roundtrip_and_unknown_id_rejected() {
    let json = serde_json::to_string(&ServiceId::NET_RELAY).unwrap();
    assert_eq!(json, "\"net.relay\"");
    let parsed: ServiceId = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, ServiceId::NET_RELAY);
    assert!(serde_json::from_str::<ServiceId>("\"owner.superuser\"").is_err());
}

#[test]
fn registry_upsert_and_last_write_wins() {
    let mut reg = ServiceRegistry::new(vec![
        ServiceEntry {
            service_id: ServiceId::SERVE_ACP,
            enabled: false,
            updated_at: 1,
        },
        ServiceEntry {
            service_id: ServiceId::SERVE_ACP,
            enabled: true,
            updated_at: 2,
        },
    ]);
    assert_eq!(reg.entries().len(), 1, "同 id 归一为后写者胜");
    assert_eq!(reg.file_value(ServiceId::SERVE_ACP), Some(true));
    reg.set_enabled(ServiceId::SERVE_ACP, false, 9);
    let entry = reg.entries().first().unwrap();
    assert!(!entry.enabled);
    assert_eq!(entry.updated_at, 9);
    reg.set_enabled(ServiceId::NET_RELAY, false, 10);
    assert_eq!(reg.entries().len(), 2);
    assert_eq!(reg.file_value(ServiceId::SERVE_TUNNEL), None);
}
