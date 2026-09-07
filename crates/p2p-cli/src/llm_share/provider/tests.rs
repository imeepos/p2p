use super::*;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pcli-provider-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn params(id: Option<&str>, name: &str, models: &[&str]) -> SaveParams {
    SaveParams {
        id: id.map(str::to_owned),
        name: name.to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        protocol: Protocol::OpenAI,
        api_key: "sk-test-secret-1234567890".to_owned(),
        models: models.iter().map(|m| m.to_string()).collect(),
        created_at: 1_000,
    }
}

#[test]
fn save_list_get_remove_roundtrip() {
    let dir = temp_dir("roundtrip");
    let a = save(dir.to_str().unwrap(), params(Some("prov-a"), "A", &["gpt-4o"])).unwrap();
    assert_eq!(a.id, "prov-a");
    let b = save(
        dir.to_str().unwrap(),
        SaveParams {
            protocol: Protocol::Claude,
            ..params(Some("prov-b"), "B", &["claude-3"])
        },
    )
    .unwrap();
    assert_eq!(b.protocol, Protocol::Claude);
    let views = list(dir.to_str().unwrap()).unwrap();
    assert_eq!(views.len(), 2);
    let a_view = views.iter().find(|v| v.id == "prov-a").unwrap();
    assert_eq!(a_view.api_key_masked, mask_key("sk-test-secret-1234567890"));
    assert!(!a_view.api_key_masked.contains("sk-test-secret-1234567890"));
    assert_eq!(get(dir.to_str().unwrap(), "prov-a").unwrap().unwrap().name, "A");
    assert!(remove(dir.to_str().unwrap(), "prov-a").unwrap());
    assert!(!key_path(dir.to_str().unwrap(), "prov-a").exists(), "级联删密钥文件");
    assert_eq!(get(dir.to_str().unwrap(), "prov-a").unwrap(), None);
    assert!(key_path(dir.to_str().unwrap(), "prov-b").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn key_file_is_0600_and_dirs_0700() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("perms");
    save(dir.to_str().unwrap(), params(Some("prov-p"), "P", &["gpt-4o"])).unwrap();
    let key_file = key_path(dir.to_str().unwrap(), "prov-p");
    let mode = std::fs::metadata(&key_file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "密钥文件必须 0600");
    let keys_dir = file_path(dir.to_str().unwrap(), KEYS_DIR);
    assert_eq!(std::fs::metadata(&keys_dir).unwrap().permissions().mode() & 0o777, 0o700);
    let llm_share_dir = file_path(dir.to_str().unwrap(), "");
    assert_eq!(
        std::fs::metadata(&llm_share_dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_archive_is_explicit_error() {
    let dir = temp_dir("corrupt");
    let file = path(dir.to_str().unwrap());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{ not json").unwrap();
    assert!(list(dir.to_str().unwrap()).is_err());
    assert!(save(dir.to_str().unwrap(), params(Some("x"), "X", &["m"])).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unsupported_version_is_explicit_error() {
    let dir = temp_dir("version");
    let file = path(dir.to_str().unwrap());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, r#"{"v":2,"providers":{}}"#).unwrap();
    assert!(list(dir.to_str().unwrap()).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn model_conflict_between_providers_rejected() {
    let dir = temp_dir("conflict");
    save(dir.to_str().unwrap(), params(Some("prov-a"), "A", &["gpt-4o"])).unwrap();
    let err = save(dir.to_str().unwrap(), params(Some("prov-b"), "B", &["gpt-4o", "claude-3"]))
        .unwrap_err();
    assert!(err.contains("占用"), "{err}");
    assert_eq!(list(dir.to_str().unwrap()).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn required_fields_are_validated() {
    let dir = temp_dir("required");
    assert!(save(dir.to_str().unwrap(), SaveParams { name: "".into(), ..params(Some("a"), "A", &["m"]) }).is_err());
    assert!(save(dir.to_str().unwrap(), SaveParams { base_url: "  ".into(), ..params(Some("a"), "A", &["m"]) }).is_err());
    assert!(save(dir.to_str().unwrap(), SaveParams { api_key: "".into(), ..params(Some("a"), "A", &["m"]) }).is_err());
    assert!(save(dir.to_str().unwrap(), SaveParams { models: vec![], ..params(Some("a"), "A", &["m"]) }).is_err());
    assert!(save(dir.to_str().unwrap(), SaveParams { models: vec!["  ".into()], ..params(Some("a"), "A", &["m"]) }).is_err());
    assert!(list(dir.to_str().unwrap()).unwrap().is_empty(), "失败路径零落盘");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mask_key_shapes() {
    assert_eq!(mask_key("short"), "****");
    assert_eq!(mask_key("abcdefgh"), "****");
    assert_eq!(mask_key("abcdefghijkl"), "abcd****ijkl");
    assert_eq!(mask_key(""), "****");
}
