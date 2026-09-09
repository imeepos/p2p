//! authz import a2a 测试（§9 验收矩阵）：映射 operator / 幂等 / grants 表保留 /
//! 无 grant 不导入不升级 / 非法 peer 列报 / 空簿与损坏显式语义。

use super::*;

fn temp_dir(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "import-a2a-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ))
        .to_string_lossy()
        .into_owned()
}

/// 确定性 32 字节 base58 PeerId（逐字节 fill）。
fn peer_id(fill: u8) -> String {
    bs58::encode([fill; 32]).into_string()
}

fn write_grants(dir: &str, body: &str) -> std::path::PathBuf {
    let path = Path::new(dir).join(GRANTS_FILE);
    std::fs::create_dir_all(Path::new(dir)).expect("tmp dir");
    std::fs::write(&path, body).expect("write grants");
    path
}

fn grants_body(peers: &[&str]) -> String {
    let entries: Vec<String> = peers
        .iter()
        .enumerate()
        .map(|(i, p)| format!(r#"{{ "agentId": "agent-{i}", "peer": "{p}", "grantedAt": 100 }}"#))
        .collect();
    format!("{{ \"version\": 1, \"grants\": [{}] }}", entries.join(", "))
}

fn invoke_decision(dir: &str, peer: &str) -> String {
    Authz::new(Path::new(dir), SystemClock)
        .check(peer, Permission::A2A_INVOKE)
        .map(|d| d.to_string())
        .unwrap_or_else(|e| format!("Err({e})"))
}

#[test]
fn granted_peers_map_to_operator_and_allow_invoke() {
    let dir = temp_dir("map");
    let (a, b) = (peer_id(1), peer_id(2));
    write_grants(&dir, &grants_body(&[&a, &b]));
    let report = import_a2a(&dir).expect("import");
    assert_eq!(report.peers_total, 2);
    assert_eq!(report.bound, 2);
    assert_eq!(invoke_decision(&dir, &a), "Allow");
    assert_eq!(invoke_decision(&dir, &b), "Allow");
}

#[test]
fn rerun_is_idempotent_and_keeps_bindings_untouched() {
    let dir = temp_dir("idem");
    let a = peer_id(3);
    write_grants(&dir, &grants_body(&[&a]));
    let first = import_a2a(&dir).expect("first");
    assert_eq!(first.bound, 1);
    let bindings =
        std::fs::read(Path::new(&dir).join("authz").join("bindings.json")).expect("bindings");
    let second = import_a2a(&dir).expect("second");
    assert_eq!((second.bound, second.kept), (0, 1), "重跑全 kept");
    let after =
        std::fs::read(Path::new(&dir).join("authz").join("bindings.json")).expect("bindings after");
    assert_eq!(bindings, after, "重跑不得翻新绑定表");
}

#[test]
fn grants_file_stays_intact_after_import() {
    let dir = temp_dir("keep");
    let a = peer_id(4);
    let path = write_grants(&dir, &grants_body(&[&a]));
    let before = std::fs::read(&path).expect("grants before");
    import_a2a(&dir).expect("import");
    let after = std::fs::read(&path).expect("grants after");
    assert_eq!(before, after, "grants 表原处保留，双查不删");
}

#[test]
fn peers_without_grant_are_not_imported_nor_upgraded() {
    let dir = temp_dir("nogrant");
    let (granted, bound_guest) = (peer_id(5), peer_id(6));
    write_grants(&dir, &grants_body(&[&granted]));
    let authz = Authz::new(Path::new(&dir), SystemClock);
    authz
        .bind(&bound_guest, "guest", None, "pre")
        .expect("bind");
    let report = import_a2a(&dir).expect("import");
    assert_eq!(report.peers_total, 1, "无 grant 的 peer 不进导入集");
    assert!(
        !report.outcomes.iter().any(|o| o.peer == bound_guest),
        "已绑 guest 无 grant：不得升级为 operator"
    );
    assert_eq!(invoke_decision(&dir, &bound_guest), "Deny(MissingPerm)");
}

#[test]
fn invalid_peer_listed_and_valid_peers_still_imported() {
    let dir = temp_dir("invalid");
    let a = peer_id(7);
    write_grants(&dir, &grants_body(&[&a, "not-a-peer"]));
    let report = import_a2a(&dir).expect("import");
    assert_eq!((report.bound, report.invalid), (1, 1));
    let bad = report
        .outcomes
        .iter()
        .find(|o| o.state == "invalid")
        .expect("invalid outcome");
    assert!(bad.detail.contains("PeerId 非法"));
}

#[test]
fn missing_grants_file_is_empty_success() {
    let dir = temp_dir("empty");
    let report = import_a2a(&dir).expect("import");
    assert_eq!((report.grants_total, report.peers_total), (0, 0));
}

#[test]
fn corrupted_grants_file_fails_loudly() {
    let dir = temp_dir("corrupt");
    write_grants(&dir, "{not json");
    let err = import_a2a(&dir).expect_err("必须显式失败");
    assert!(err.contains("parse"), "实得: {err}");
}
