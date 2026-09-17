//! ftp_config 命令面单测（红绿双向）：缺失/损坏回退、空密码保留、0600、
//! 原子写、整表替换、新账号空密码拒存。内核直测（&Path），不经 Tauri State。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::*;

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-console-ftpcfg-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

fn accounts(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(u, p)| (u.to_string(), p.to_string()))
        .collect()
}

#[test]
fn get_missing_file_returns_empty_view() {
    let dir = temp_root("missing");
    let view = load_view(&dir.join(FILE_NAME));
    assert_eq!(
        view,
        FtpConfigView::default(),
        "契约：缺失回空值 root=\"\"/authz=false/users=[]"
    );
}

#[test]
fn get_corrupt_file_returns_empty_view() {
    let dir = temp_root("corrupt");
    fs::write(dir.join(FILE_NAME), b"{not json").expect("write ok");
    assert_eq!(load_view(&dir.join(FILE_NAME)), FtpConfigView::default());
}

#[test]
fn get_returns_sorted_usernames_without_secrets() {
    let dir = temp_root("users");
    fs::write(
        dir.join(FILE_NAME),
        r#"{"root":"/srv/ftp","accounts":{"bob":"pw2","alice":"pw1"},"authz":true}"#,
    )
    .expect("write ok");
    let view = load_view(&dir.join(FILE_NAME));
    assert_eq!(view.root, "/srv/ftp");
    assert!(view.authz);
    assert_eq!(view.users, vec!["alice", "bob"], "仅用户名且排序");
    let raw = serde_json::to_string(&view).unwrap();
    assert!(!raw.contains("pw1") && !raw.contains("pw2"), "密码不回显: {raw}");
}

#[test]
fn save_keeps_existing_password_for_blank_and_replaces_table() {
    let dir = temp_root("blank");
    let path = dir.join(FILE_NAME);
    save_config(&path, "/srv/ftp", false, &accounts(&[("alice", "old"), ("carol", "c")]))
        .expect("首次保存 ok");
    // alice 空密码 = 保留；表内不含 carol = 整表替换移除。
    save_config(&path, "/srv/ftp2", true, &accounts(&[("alice", "")]))
        .expect("二次保存 ok");
    let raw = fs::read_to_string(&path).unwrap();
    let cfg: FtpConfigFile = serde_json::from_str(&raw).unwrap();
    assert_eq!(cfg.root, "/srv/ftp2");
    assert!(cfg.authz);
    assert_eq!(cfg.accounts.len(), 1, "整表替换：carol 不在表内");
    assert_eq!(cfg.accounts["alice"], "old", "空密码保留原密码");
}

#[test]
fn save_rejects_blank_password_for_new_account() {
    let dir = temp_root("newblank");
    let err = save_config(&dir.join(FILE_NAME), "/srv", false, &accounts(&[("nobody", "")]))
        .unwrap_err();
    assert!(err.contains("nobody") && err.contains("密码不能为空"), "{err}");
    assert!(!dir.join(FILE_NAME).exists(), "拒存不落盘");
}

#[test]
fn save_rejects_corrupt_existing_without_overwrite() {
    let dir = temp_root("savecorrupt");
    let path = dir.join(FILE_NAME);
    fs::write(&path, b"{broken").expect("write ok");
    assert!(save_config(&path, "/srv", false, &accounts(&[("u", "p")])).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{broken", "损坏文件不被静默覆盖");
}

#[test]
fn save_is_atomic_0600_and_roundtrips() {
    let dir = temp_root("atomic");
    let path = dir.join(FILE_NAME);
    // 预置合法旧文件（root 必填，与 CLI 镜像一致）+ 宽松权限，验证保存收敛 0600。
    fs::write(&path, r#"{"root":"/old","accounts":{}}"#).expect("预置旧文件");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }
    save_config(&path, "/srv/ftp", true, &accounts(&[("alice", "pw")])).expect("save ok");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "现状非 0600 亦在保存时收敛为 0600");
    }
    assert!(!dir.join("ftp.json.tmp").exists(), "成功写无 tmp 残留");
    let view = load_view(&path);
    assert_eq!(view.root, "/srv/ftp");
    assert!(view.authz);
    assert_eq!(view.users, vec!["alice"]);
}
