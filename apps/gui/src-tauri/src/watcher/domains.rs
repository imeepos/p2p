//! 数据域归类（W1 R1）：文件路径 → 数据域，路径语义以 store 层为准。
//!
//! 白名单外（key.seed、control/、p2p-data/…）一律不归类，配合非递归挂载
//! 双保险防全目录递归风暴；原子写临时文件（同前缀 .tmp）归同域。

use std::path::{Path, PathBuf};

use serde::Serialize;

/// GUI 前端定向刷新的数据域（data-changed.domains，R2 按域重载）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataDomain {
    Config,
    Profile,
    Chat,
}

impl DataDomain {
    pub fn key(self) -> &'static str {
        match self {
            DataDomain::Config => "config",
            DataDomain::Profile => "profile",
            DataDomain::Chat => "chat",
        }
    }
}

/// 监听目标：数据目录内三个关键文件（config/profile/好友簿），非递归。
#[derive(Clone)]
pub struct WatchTargets {
    /// app 数据目录本体（gui-config.json / node-profile.json 所在）。
    pub app_dir: PathBuf,
    /// 好友簿目录（app_dir/chat；启动缺失时由消费线程懒挂）。
    pub chat_dir: PathBuf,
}

pub fn targets(app_data_dir: &Path) -> WatchTargets {
    WatchTargets {
        app_dir: app_data_dir.to_path_buf(),
        chat_dir: app_data_dir.join("chat"),
    }
}

/// friends.json 路径语义对齐 p2p-chat store（data_dir/chat/friends.json）。
pub fn friends_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("chat").join("friends.json")
}

/// 路径 → 数据域：白名单文件名前缀归类；config/profile 的原子写临时文件
/// （gui-config.json.tmp）同前缀自然归同域；目录与其余文件返回 None。
pub fn classify(path: &Path) -> Option<DataDomain> {
    let name = path.file_name()?.to_string_lossy();
    if name.starts_with(crate::config::FILE_NAME) {
        return Some(DataDomain::Config);
    }
    if name.starts_with(crate::profile::FILE_NAME) {
        return Some(DataDomain::Profile);
    }
    if name.starts_with("friends.json") {
        return Some(DataDomain::Chat);
    }
    None
}

/// 一批防抖事件路径 → 去重排序的域列表（无关事件整批返回空）。
pub fn collect_domains(paths: impl IntoIterator<Item = PathBuf>) -> Vec<DataDomain> {
    let mut domains: Vec<DataDomain> = paths.into_iter().filter_map(|p| classify(&p)).collect();
    domains.sort();
    domains.dedup();
    domains
}

/// data-changed 事件载荷：domains 驱动前端定向重载（禁全应用刷新）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DataChanged {
    pub domains: Vec<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        PathBuf::from("/tmp/w1-test").join(name)
    }

    #[test]
    fn classify_maps_whitelist_files() {
        assert_eq!(classify(&dir("app").join("gui-config.json")), Some(DataDomain::Config));
        // 原子写临时文件与正式文件同域
        assert_eq!(
            classify(&dir("app").join("gui-config.json.tmp")),
            Some(DataDomain::Config)
        );
        assert_eq!(
            classify(&dir("app").join("node-profile.json")),
            Some(DataDomain::Profile)
        );
        assert_eq!(
            classify(&dir("app").join("chat").join("friends.json")),
            Some(DataDomain::Chat)
        );
    }

    #[test]
    fn classify_rejects_non_whitelist() {
        // 递归风暴防护：数据目录其余文件/目录一律不归类
        assert_eq!(classify(&dir("app").join("key.seed")), None);
        assert_eq!(classify(&dir("app").join("control").join("token")), None);
        assert_eq!(classify(&dir("app").join("chat").join("abc.jsonl")), None);
        assert_eq!(classify(&dir("app").join("chat")), None);
        assert_eq!(classify(&dir("app")), None);
    }

    #[test]
    fn collect_domains_dedups_and_sorts() {
        let paths = vec![
            dir("app").join("chat").join("friends.json"),
            dir("app").join("gui-config.json"),
            dir("app").join("gui-config.json.tmp"),
            dir("app").join("node-profile.json"),
            dir("app").join("key.seed"),
        ];
        assert_eq!(
            collect_domains(paths),
            vec![DataDomain::Config, DataDomain::Profile, DataDomain::Chat]
        );
        assert!(collect_domains(Vec::new()).is_empty());
    }

    #[test]
    fn data_changed_payload_serializes_domain_keys() {
        let payload = DataChanged {
            domains: vec![DataDomain::Config.key(), DataDomain::Chat.key()],
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "domains": ["config", "chat"] })
        );
    }

    #[test]
    fn targets_follow_store_layout() {
        let t = targets(Path::new("/data"));
        assert_eq!(t.app_dir, PathBuf::from("/data"));
        assert_eq!(t.chat_dir, PathBuf::from("/data/chat"));
        assert_eq!(friends_path(Path::new("/data")), PathBuf::from("/data/chat/friends.json"));
    }
}
