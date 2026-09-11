//! descriptor 约定路径与读写用例：0600 落盘、原子替换、损坏显式报错。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::{
    read_descriptor, write_descriptor, LocalAgentDescriptor, DESCRIPTOR_FILE, DESCRIPTOR_SUBDIR,
};

fn sample() -> LocalAgentDescriptor {
    LocalAgentDescriptor {
        version: 1,
        admin_url: "http://127.0.0.1:8123".into(),
        token: "abcd1234".into(),
        peer: "12D3KooWTest".into(),
        agent_name: "home-agent".into(),
        written_at_unix: 1_725_700_000,
    }
}

fn temp_home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acp-desc-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp home");
    dir
}

#[test]
fn descriptor_path_is_under_dsh_acp() {
    let home = PathBuf::from("/tmp/acp-desc-home");
    assert_eq!(
        super::descriptor_path(&home),
        home.join(DESCRIPTOR_SUBDIR).join(DESCRIPTOR_FILE)
    );
}

#[test]
fn write_then_read_roundtrip_and_mode_is_0600() {
    let home = temp_home("roundtrip");
    let path = write_descriptor(&home, &sample()).expect("write");
    assert!(path.ends_with(DESCRIPTOR_FILE));
    let mode = fs::metadata(&path).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "token 明文文件必须 0600");
    assert_eq!(read_descriptor(&path).expect("read"), sample());
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn rewrite_replaces_atomically_without_tmp_leftover() {
    let home = temp_home("rewrite");
    write_descriptor(&home, &sample()).expect("write 1");
    let mut next = sample();
    next.admin_url = "http://127.0.0.1:9999".into();
    write_descriptor(&home, &next).expect("write 2");
    assert_eq!(
        read_descriptor(&super::descriptor_path(&home)).expect("read"),
        next
    );
    let entries: Vec<_> = fs::read_dir(home.join(DESCRIPTOR_SUBDIR))
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(entries.len(), 1, "tmp 文件不得残留：{:?}", entries);
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn read_missing_and_malformed_are_explicit() {
    let home = temp_home("errors");
    assert!(matches!(
        read_descriptor(&super::descriptor_path(&home)),
        Err(super::DescriptorError::Missing)
    ));
    fs::create_dir_all(home.join(DESCRIPTOR_SUBDIR)).expect("dir");
    fs::write(super::descriptor_path(&home), "{ not json").expect("bad file");
    assert!(matches!(
        read_descriptor(&super::descriptor_path(&home)),
        Err(super::DescriptorError::Malformed(_))
    ));
    let _ = fs::remove_dir_all(&home);
}
