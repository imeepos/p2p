//! GUI 日志目录解析与 frontend.log 读取/清理：语义逐条对齐 apps/gui frontend_log.rs
//! （默认 200 行、上限 1000、1MB 轮转单代、clear 连轮转代一起删且幂等）。
//! 默认目录取 Tauri app_log_dir（identifier com.p2p.console）的平台路径。

use std::path::{Path, PathBuf};

/// GUI 应用标识（tauri.conf.json identifier），决定默认日志目录。
pub const GUI_IDENTIFIER: &str = "com.p2p.console";
pub const FRONTEND_LOG_FILE: &str = "frontend.log";
pub const ROTATED_FILE: &str = "frontend.log.1";
pub const MAX_TAIL_LINES: usize = 1000;
pub const DEFAULT_TAIL_LINES: u32 = 200;

/// frontend.log 视图：路径 + 读取/清理（多进程并发由文件语义兜底，无 GUI 内写锁）。
pub struct FrontendLog {
    path: PathBuf,
}

impl FrontendLog {
    /// log_dir 显式给出则直接用（E2E/测试隔离）；否则解析 GUI 默认日志目录。
    pub fn resolve(log_dir: Option<&str>) -> Result<Self, String> {
        let dir = match log_dir {
            Some(dir) => PathBuf::from(dir),
            None => default_gui_log_dir()?,
        };
        Ok(Self { path: dir.join(FRONTEND_LOG_FILE) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 读末尾 max 行；文件不存在返回空（GUI 首启前合法状态，不算错误）。
    pub fn tail(&self, max_lines: usize) -> std::io::Result<Vec<String>> {
        let max = max_lines.min(MAX_TAIL_LINES);
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let lines: Vec<String> = read_lines(file)?;
        let start = lines.len().saturating_sub(max);
        Ok(lines[start..].to_vec())
    }

    /// 清理 frontend.log 与 frontend.log.1；返回 (删了当前代, 删了轮转代)。幂等。
    pub fn clear(&self) -> std::io::Result<(bool, bool)> {
        Ok((
            remove_if_exists(&self.path)?,
            remove_if_exists(&self.path.with_file_name(ROTATED_FILE))?,
        ))
    }
}

fn read_lines(file: std::fs::File) -> std::io::Result<Vec<String>> {
    use std::io::BufRead;
    std::io::BufReader::new(file).lines().collect()
}

/// 删除文件；NotFound 视为已清理，其余错误上抛（禁止静默吞错）。
fn remove_if_exists(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Tauri app_log_dir 平台等价解析（identifier 固定 com.p2p.console）：
/// macOS ~/Library/Logs/<id>；Linux XDG_STATE_HOME|~/.local/state 下 <id>/logs；
/// Windows %LOCALAPPDATA%<id>/logs。无法定位时显式报错，提示用 --log-dir 指定。
pub fn default_gui_log_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home
            .map(|h| h.join("Library").join("Logs").join(GUI_IDENTIFIER))
            .ok_or_else(missing_home);
    }
    if cfg!(target_os = "linux") {
        return home
            .map(|h| {
                let state = std::env::var_os("XDG_STATE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| h.join(".local").join("state"));
                state.join(GUI_IDENTIFIER).join("logs")
            })
            .ok_or_else(missing_home);
    }
    if cfg!(target_os = "windows") {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local).join(GUI_IDENTIFIER).join("logs"));
        }
    }
    Err(missing_home())
}

fn missing_home() -> String {
    format!(
        "无法定位 GUI 日志目录（HOME/LOCALAPPDATA 未设置），请用 --log-dir 显式指定（标识 {GUI_IDENTIFIER}）"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("p2pctl-logb-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tail_returns_last_lines_and_caps() {
        let dir = temp_dir("tail");
        let log = FrontendLog::resolve(dir.to_str()).unwrap();
        std::fs::write(log.path(), "1\n2\n3\n").unwrap();
        assert_eq!(log.tail(2).unwrap(), vec!["2", "3"]);
        assert_eq!(log.tail(MAX_TAIL_LINES + 5).unwrap().len(), 3, "上限钳制到 1000");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_missing_file_is_empty() {
        let dir = temp_dir("missing");
        let log = FrontendLog::resolve(dir.to_str()).unwrap();
        assert!(log.tail(5).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_removes_both_generations_and_is_idempotent() {
        let dir = temp_dir("clear");
        let log = FrontendLog::resolve(dir.to_str()).unwrap();
        std::fs::write(log.path(), "stale").unwrap();
        std::fs::write(log.path().with_file_name(ROTATED_FILE), "old").unwrap();
        assert_eq!(log.clear().unwrap(), (true, true));
        assert_eq!(log.clear().unwrap(), (false, false), "幂等：缺文件不算错");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_honors_explicit_dir() {
        let log = FrontendLog::resolve(Some("/tmp/whatever")).unwrap();
        assert_eq!(log.path(), Path::new("/tmp/whatever/frontend.log"));
    }
}
