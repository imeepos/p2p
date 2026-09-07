//! 本机 agent 自描述读取（2026-09-07 用户裁决：分享流免手填 admin token）：
//! 读 acp-agent 落盘的用户级约定文件（~/.dsh/acp/local-agent.json，0600）。
//! 缺失/损坏返回 null：缺失是常态（远端场景/agent 未跑），损坏告警留痕不静默；
//! 两者都不阻断 GUI 主功能。

use serde::Serialize;

use acp_common::{descriptor_path, read_descriptor, user_home_dir, DescriptorError};

/// 前端视图（camelCase，契约面风格）；token 原文仅经内存透传前端，不落日志。
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAdminDescriptorView {
    pub admin_url: String,
    pub token: String,
    pub peer: String,
    pub agent_name: String,
    pub written_at_unix: u64,
}

/// 读取实现（home 注入便于测试）；None = 无可用自描述。
pub fn load_local_descriptor(home: Option<std::path::PathBuf>) -> Option<LocalAdminDescriptorView> {
    let home = home?;
    match read_descriptor(&descriptor_path(&home)) {
        Ok(descriptor) => Some(LocalAdminDescriptorView {
            admin_url: descriptor.admin_url,
            token: descriptor.token,
            peer: descriptor.peer,
            agent_name: descriptor.agent_name,
            written_at_unix: descriptor.written_at_unix,
        }),
        Err(DescriptorError::Missing) => None,
        Err(err) => {
            tracing::warn!(error = %err, "本机 agent 描述文件不可读：按缺失处理");
            None
        }
    }
}

/// 契约命令：无本机描述（agent 未跑/远端 agent）返回 null，前端回落手动登记。
#[tauri::command]
pub fn acp_local_descriptor() -> Option<LocalAdminDescriptorView> {
    load_local_descriptor(user_home_dir())
}

#[cfg(test)]
mod tests {
    use super::load_local_descriptor;
    use acp_common::{descriptor_path, write_descriptor, LocalAgentDescriptor};

    fn temp_home(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("acp-desc-cmd-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp home");
        dir
    }

    fn sample() -> LocalAgentDescriptor {
        LocalAgentDescriptor {
            version: acp_common::DESCRIPTOR_VERSION,
            admin_url: "http://127.0.0.1:8123".into(),
            token: "abcd1234".into(),
            peer: "12D3KooW".into(),
            agent_name: "home-agent".into(),
            written_at_unix: 1_725_700_000,
        }
    }

    #[test]
    fn reads_descriptor_into_camel_view() {
        let home = temp_home("read");
        write_descriptor(&home, &sample()).expect("write");
        let view = load_local_descriptor(Some(home.clone())).expect("some");
        assert_eq!(view.admin_url, "http://127.0.0.1:8123");
        assert_eq!(view.token, "abcd1234");
        assert_eq!(view.peer, "12D3KooW");
        assert_eq!(view.written_at_unix, 1_725_700_000);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn missing_home_and_missing_file_are_none() {
        assert_eq!(load_local_descriptor(None), None);
        let home = temp_home("missing");
        assert_eq!(load_local_descriptor(Some(home.clone())), None);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn malformed_descriptor_degrades_to_none() {
        let home = temp_home("malformed");
        std::fs::create_dir_all(home.join(".dsh/acp")).expect("dir");
        std::fs::write(descriptor_path(&home), "{ not json").expect("bad file");
        assert_eq!(load_local_descriptor(Some(home.clone())), None);
        let _ = std::fs::remove_dir_all(&home);
    }
}
