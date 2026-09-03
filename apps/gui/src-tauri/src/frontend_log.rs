//! 前端错误落盘（契约 v3 加法，G-H 观测）：前端采集的 error/unhandledrejection/console.error
//! 以 JSONL 追加到 app_log_dir/frontend.log，供 Agent/人工直接读文件排障——无需打开 DevTools。

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::State;

const LOG_FILE: &str = "frontend.log";
const ROTATED_FILE: &str = "frontend.log.1";
const ROTATE_BYTES: u64 = 1_000_000;
const MAX_TAIL_LINES: usize = 1000;
const DEFAULT_TAIL_LINES: u32 = 200;

/// 前端日志状态：目标文件路径 + 写锁（多窗口并发追加串行化）。
pub struct FrontendLog {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FrontendLog {
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self {
            path: dir.join(LOG_FILE),
            lock: Mutex::new(()),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, lines: &[String]) -> std::io::Result<()> {
        let _guard = self.lock.lock().expect("frontend_log 锁中毒");
        rotate_if_needed(&self.path)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        for line in lines {
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }
        Ok(())
    }

    pub fn tail(&self, max_lines: usize) -> std::io::Result<Vec<String>> {
        let _guard = self.lock.lock().expect("frontend_log 锁中毒");
        tail_lines(&self.path, max_lines)
    }

    /// 一键清理：删除 frontend.log 与轮转代 frontend.log.1（幂等，缺文件不算错）。
    pub fn clear(&self) -> std::io::Result<()> {
        let _guard = self.lock.lock().expect("frontend_log 锁中毒");
        remove_if_exists(&self.path)?;
        remove_if_exists(&self.path.with_file_name(ROTATED_FILE))
    }
}

/// 超限轮转：frontend.log → frontend.log.1（单代覆盖），控制 tail 整读内存上界。
fn rotate_if_needed(path: &Path) -> std::io::Result<()> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(()), // 尚无文件，无需轮转
    };
    if meta.len() <= ROTATE_BYTES {
        return Ok(());
    }
    fs::rename(path, path.with_file_name(ROTATED_FILE))
}

/// 删除文件；不存在视为已清理（幂等），其余错误上抛。
fn remove_if_exists(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// 读文件末尾 max 行；文件不存在返回空（首次启动合法状态，不算错误）。
fn tail_lines(path: &Path, max_lines: usize) -> std::io::Result<Vec<String>> {
    let max = max_lines.min(MAX_TAIL_LINES);
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let lines: Vec<String> = BufReader::new(file)
        .lines()
        .collect::<std::io::Result<_>>()?;
    let start = lines.len().saturating_sub(max);
    Ok(lines[start..].to_vec())
}

/// frontend_log_append：前端错误 JSONL 批量落盘；失败返回 Err（可观测，不静默）。
#[tauri::command]
pub async fn frontend_log_append(
    state: State<'_, FrontendLog>,
    lines: Vec<String>,
) -> Result<(), String> {
    state
        .append(&lines)
        .map_err(|e| format!("前端日志写入失败: {e}"))
}

/// frontend_log_tail：读日志末尾 maxLines 行（默认 200，上限 1000）。
#[tauri::command]
pub async fn frontend_log_tail(
    state: State<'_, FrontendLog>,
    max_lines: Option<u32>,
) -> Result<Vec<String>, String> {
    let max = max_lines.unwrap_or(DEFAULT_TAIL_LINES).max(1) as usize;
    state
        .tail(max)
        .map_err(|e| format!("前端日志读取失败: {e}"))
}

/// frontend_log_path：日志绝对路径（诊断视图展示 + Agent 定位）。
#[tauri::command]
pub async fn frontend_log_path(state: State<'_, FrontendLog>) -> Result<String, String> {
    Ok(state.path().display().to_string())
}

/// frontend_log_clear：一键清理 frontend.log 与 frontend.log.1（幂等）。
#[tauri::command]
pub async fn frontend_log_clear(state: State<'_, FrontendLog>) -> Result<(), String> {
    state
        .clear()
        .map_err(|e| format!("前端日志清理失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("p2p-console-fl-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn append_then_tail_roundtrip() {
        let dir = temp_dir("roundtrip");
        let log = FrontendLog::new(&dir).unwrap();
        log.append(&["{\"k\":1}".into(), "{\"k\":2}".into(), "{\"k\":3}".into()])
            .unwrap();
        assert_eq!(log.tail(2).unwrap(), vec!("{\"k\":2}", "{\"k\":3}"));
        assert_eq!(log.tail(10).unwrap().len(), 3);
        assert!(log.path().ends_with("frontend.log"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_missing_file_is_empty() {
        let dir = temp_dir("missing");
        let log = FrontendLog::new(&dir).unwrap();
        assert!(log.tail(5).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_caps_at_limit() {
        let dir = temp_dir("cap");
        let log = FrontendLog::new(&dir).unwrap();
        let lines: Vec<String> = (0..MAX_TAIL_LINES as i32 + 5).map(|i| i.to_string()).collect();
        log.append(&lines).unwrap();
        assert_eq!(log.tail(usize::MAX).unwrap().len(), MAX_TAIL_LINES);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotate_renames_oversized_log() {
        let dir = temp_dir("rotate");
        let log = FrontendLog::new(&dir).unwrap();
        let big = "x".repeat(ROTATE_BYTES as usize + 1);
        log.append(&[big]).unwrap();
        log.append(&["after-rotate".into()]).unwrap();
        assert!(dir.join(ROTATED_FILE).exists());
        assert_eq!(log.tail(1).unwrap(), vec!["after-rotate"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_removes_current_and_rotated_then_stays_usable() {
        let dir = temp_dir("clear");
        let log = FrontendLog::new(&dir).unwrap();
        log.append(&["stale".into()]).unwrap();
        fs::write(dir.join(ROTATED_FILE), "rotated-stale").unwrap();
        log.clear().unwrap();
        assert!(!dir.join(LOG_FILE).exists());
        assert!(!dir.join(ROTATED_FILE).exists());
        // 幂等：文件已缺再清不报错；清后 append/tail 恢复正常。
        log.clear().unwrap();
        log.append(&["fresh".into()]).unwrap();
        assert_eq!(log.tail(10).unwrap(), vec!["fresh"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
