//! 节点资料持久化（契约 v6 §11）：name/description/avatar，app 数据目录 node-profile.json。
//!
//! 定位：纯 GUI 展示属性，不进底座、不随发现协议广播；保存后无需重启节点即生效。
//! 持久化形态与 config.rs 一致：原子写（tmp+rename），损坏回退默认并告警（禁止静默）。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tracing::warn;

/// name 上限（trim 后字符数，契约 §11）。
pub const NAME_MAX_CHARS: usize = 64;
/// description 上限（字符数，契约 §11）。
pub const DESCRIPTION_MAX_CHARS: usize = 280;
/// avatar data URL 总长上限（ASCII 字符数，契约 §11）。
pub const AVATAR_MAX_LEN: usize = 200_000;

/// 资料文件名。
const FILE_NAME: &str = "node-profile.json";

/// 允许的 avatar data URL 前缀（契约 §11 MIME 白名单）。
const AVATAR_MIME_PREFIXES: [&str; 3] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
];

/// 节点资料（契约 §11 NodeProfile）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NodeProfile {
    pub name: String,
    pub description: String,
    /// data URL；None 序列化为 null = 未设置。
    pub avatar: Option<String>,
}

impl NodeProfile {
    /// 契约 §11 校验；Err 一律可读中文。
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().chars().count() > NAME_MAX_CHARS {
            return Err(format!("节点名称过长，上限 {NAME_MAX_CHARS} 字符"));
        }
        if self.description.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(format!("节点描述过长，上限 {DESCRIPTION_MAX_CHARS} 字符"));
        }
        if let Some(avatar) = &self.avatar {
            validate_avatar(avatar)?;
        }
        Ok(())
    }
}

/// avatar 校验：长度上限、MIME 白名单、base64 载荷字符集。
fn validate_avatar(url: &str) -> Result<(), String> {
    if url.len() > AVATAR_MAX_LEN {
        return Err(format!("头像数据过大，上限 {AVATAR_MAX_LEN} 字符"));
    }
    let payload = AVATAR_MIME_PREFIXES
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix));
    let Some(payload) = payload else {
        return Err("头像格式不支持，仅允许 PNG/JPEG/WebP 的 base64 data URL".into());
    };
    if !payload.chars().all(is_base64_char) {
        return Err("头像数据不是合法的 base64 载荷".into());
    }
    Ok(())
}

fn is_base64_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')
}

/// 持久化读写句柄：绑定 app 数据目录，串行化写盘（形态与 config.rs 一致）。
pub struct ProfileStore {
    app_data_dir: PathBuf,
    io_lock: Mutex<()>,
}

impl ProfileStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            io_lock: Mutex::new(()),
        }
    }

    fn path(&self) -> PathBuf {
        self.app_data_dir.join(FILE_NAME)
    }

    /// 读资料：无文件返回默认值；损坏回退默认值并告警（禁止静默）。
    pub fn load(&self) -> NodeProfile {
        let path = self.path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return NodeProfile::default(),
            Err(e) => {
                warn!(error = %e, path = %path.display(), "读取节点资料失败，回退默认资料");
                return NodeProfile::default();
            }
        };
        match serde_json::from_str(&text) {
            Ok(profile) => profile,
            Err(e) => {
                warn!(error = %e, path = %path.display(), "节点资料解析失败，回退默认资料");
                NodeProfile::default()
            }
        }
    }

    /// 原子写：先写临时文件再 rename 覆盖，失败清理临时文件。
    pub fn save(&self, profile: &NodeProfile) -> Result<(), String> {
        let _io = self.io_lock.lock().expect("资料写盘锁中毒");
        let path = self.path();
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                warn!(error = %e, path = %parent.display(), "创建资料目录失败");
                return Err(format!("创建资料目录失败: {e}"));
            }
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(profile)
            .map_err(|e| format!("节点资料序列化失败: {e}"))?;
        if let Err(e) = fs::write(&tmp, text) {
            let _ = fs::remove_file(&tmp);
            warn!(error = %e, path = %tmp.display(), "写入临时资料文件失败");
            return Err(format!("写入节点资料失败: {e}"));
        }
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            warn!(error = %e, path = %path.display(), "替换资料文件失败");
            return Err(format!("保存节点资料失败: {e}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 独立临时目录：测试间互不污染，结束清理。
    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("p2p-console-profile-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    fn sample_profile() -> NodeProfile {
        NodeProfile {
            name: "家用节点".into(),
            description: "客厅常开的中继兜底节点".into(),
            avatar: Some("data:image/png;base64,aGVsbG8=".into()),
        }
    }

    #[test]
    fn missing_file_loads_default() {
        let dir = temp_root("missing");
        let store = ProfileStore::new(dir.join("app"));
        assert_eq!(store.load(), NodeProfile::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_load_roundtrip_cleans_tmp_and_overwrites() {
        let dir = temp_root("roundtrip");
        let store = ProfileStore::new(dir.join("app"));
        store.save(&sample_profile()).expect("保存资料");
        assert!(!dir.join("app").join("node-profile.json.tmp").exists());
        assert_eq!(store.load(), sample_profile());
        let mut next = sample_profile();
        next.name = "改名了".into();
        next.avatar = None;
        store.save(&next).expect("覆盖保存");
        assert_eq!(store.load(), next);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_file_falls_back_to_default() {
        let dir = temp_root("corrupt");
        let app = dir.join("app");
        fs::create_dir_all(&app).expect("创建 app 目录");
        fs::write(app.join("node-profile.json"), "{ not json").expect("写入坏文件");
        let store = ProfileStore::new(app);
        assert_eq!(store.load(), NodeProfile::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_json_fills_missing_fields_with_defaults() {
        let dir = temp_root("partial");
        let app = dir.join("app");
        fs::create_dir_all(&app).expect("创建 app 目录");
        fs::write(
            app.join("node-profile.json"),
            serde_json::json!({ "name": "只有名字" }).to_string(),
        )
        .expect("写入部分资料");
        let store = ProfileStore::new(app);
        let profile = store.load();
        assert_eq!(profile.name, "只有名字");
        assert_eq!(profile.description, "");
        assert_eq!(profile.avatar, None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_accepts_boundary_values() {
        let mut profile = sample_profile();
        profile.name = "n".repeat(NAME_MAX_CHARS);
        profile.description = "d".repeat(DESCRIPTION_MAX_CHARS);
        let prefix_len = AVATAR_MIME_PREFIXES[2].len();
        let long_avatar = format!(
            "data:image/webp;base64,{}",
            "a".repeat(AVATAR_MAX_LEN - prefix_len)
        );
        profile.avatar = Some(long_avatar);
        assert_eq!(profile.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_oversized_name_and_description() {
        let mut profile = sample_profile();
        profile.name = "名".repeat(NAME_MAX_CHARS + 1);
        assert!(profile.validate().is_err());
        profile.name = "ok".into();
        profile.description = "述".repeat(DESCRIPTION_MAX_CHARS + 1);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn validate_rejects_bad_avatar() {
        let mut profile = sample_profile();
        profile.avatar = Some("data:image/gif;base64,aGVsbG8=".into());
        assert!(profile.validate().is_err(), "MIME 白名单外拒绝");
        profile.avatar = Some("data:image/png;base64,###".into());
        assert!(profile.validate().is_err(), "非法 base64 字符拒绝");
        profile.avatar = Some(format!(
            "data:image/png;base64,{}",
            "a".repeat(AVATAR_MAX_LEN)
        ));
        assert!(profile.validate().is_err(), "超长拒绝");
        profile.avatar = Some("data:image/png".into());
        assert!(profile.validate().is_err(), "缺 base64 载荷拒绝");
    }
}
