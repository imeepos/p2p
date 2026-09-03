//! 平台标准日志目录推断。
//!
//! macOS：~/Library/Logs/<app>；Windows：%LOCALAPPDATA%\<app>\logs；
//! Linux/其他 Unix：$XDG_STATE_HOME/<app>/logs，缺省 ~/.local/state/<app>/logs。

use std::path::{Path, PathBuf};

/// 推断应用日志目录；无法定位（缺 HOME/LOCALAPPDATA 等关键环境变量）返回 None。
pub fn default_log_dir(app: &str) -> Option<PathBuf> {
    default_log_dir_with(app, &|key| {
        std::env::var_os(key).map(|v| v.to_string_lossy().into_owned())
    })
}

/// 注入环境变量查找器的纯逻辑版本，便于测试。
pub fn default_log_dir_with(app: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let home = env("HOME")?;
        Some(Path::new(&home).join("Library").join("Logs").join(app))
    } else if cfg!(windows) {
        let local = env("LOCALAPPDATA")?;
        Some(Path::new(&local).join(app).join("logs"))
    } else {
        xdg_state_dir(env).map(|base| base.join(app).join("logs"))
    }
}

/// XDG state 目录：$XDG_STATE_HOME（非空）优先，缺省 ~/.local/state。
fn xdg_state_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(state) = env("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(state));
    }
    env("HOME").map(|home| Path::new(&home).join(".local").join("state"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn macos_branch_uses_home_library_logs() {
        if !cfg!(target_os = "macos") {
            return;
        }
        let env = env_of(&[("HOME", "/Users/lab")]);
        let dir = default_log_dir_with("p2p-cli", &env).unwrap();
        assert_eq!(dir, Path::new("/Users/lab/Library/Logs/p2p-cli"));
    }

    #[test]
    fn missing_home_yields_none() {
        if !cfg!(target_os = "macos") {
            return;
        }
        assert!(default_log_dir_with("app", &|_| None).is_none());
    }

    #[test]
    fn xdg_state_home_takes_priority_when_non_empty() {
        let env = env_of(&[("XDG_STATE_HOME", "/var/state"), ("HOME", "/h")]);
        assert_eq!(xdg_state_dir(&env), Some(PathBuf::from("/var/state")));
    }

    #[test]
    fn xdg_falls_back_to_home_local_state() {
        let env = env_of(&[("HOME", "/h")]);
        assert_eq!(xdg_state_dir(&env), Some(PathBuf::from("/h/.local/state")));
        let empty = env_of(&[("XDG_STATE_HOME", ""), ("HOME", "/h")]);
        assert_eq!(
            xdg_state_dir(&empty),
            Some(PathBuf::from("/h/.local/state")),
            "空 XDG_STATE_HOME 视为未设置"
        );
    }
}
