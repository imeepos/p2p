//! 按大小滚动、按份数封顶的日志文件写入器。
//!
//! 语义：当前文件写满 max_bytes 后滚动为 .1，旧归档依次后移；
//! 归档份数超过 max_files 时丢弃最老。盘上文件总量 <= 1 + max_files。
//! 单条事件内部不再切分——文件实际大小至多越界一条事件，封顶仍成立。

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use tracing_subscriber::fmt::MakeWriter;

/// 滚动日志文件。可被 tracing-subscriber 的 with_writer 直接使用。
#[derive(Debug)]
pub struct RollingFile {
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    file: Option<File>,
    base: PathBuf,
    written: u64,
    max_bytes: u64,
    max_files: usize,
}

impl RollingFile {
    /// 打开（不存在则创建，追加模式）以 base 为当前文件的滚动写入器。
    pub fn new(base: PathBuf, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        if max_bytes == 0 || max_files == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("max_bytes/max_files 必须为正，得到 {max_bytes}/{max_files}"),
            ));
        }
        if let Some(dir) = base.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&base)?;
        let written = file.metadata()?.len();
        Ok(Self {
            state: Mutex::new(State {
                file: Some(file),
                base,
                written,
                max_bytes,
                max_files,
            }),
        })
    }

    fn lock_state(&self) -> MutexGuard<'_, State> {
        // 毒锁恢复：写入方 panic 后日志通道仍须继续可用（可观测优先）。
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl State {
    /// 滚动失败只告警一次并重置计数，避免每个事件都重试刷屏 stderr。
    fn rotate_if_needed(&mut self) {
        if self.written < self.max_bytes {
            return;
        }
        if let Err(e) = self.rotate() {
            eprintln!("p2p-log: 日志滚动失败，继续写当前文件: {e}");
            self.written = 0;
        }
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file = None;
        let oldest = self.suffixed(self.max_files);
        if oldest.exists() {
            std::fs::remove_file(&oldest)?;
        }
        for i in (1..self.max_files).rev() {
            let from = self.suffixed(i);
            if from.exists() {
                std::fs::rename(&from, self.suffixed(i + 1))?;
            }
        }
        std::fs::rename(&self.base, self.suffixed(1))?;
        let file = OpenOptions::new().create(true).append(true).open(&self.base)?;
        self.written = 0;
        self.file = Some(file);
        Ok(())
    }

    fn suffixed(&self, i: usize) -> PathBuf {
        let mut name = self.base.clone().into_os_string();
        name.push(format!(".{i}"));
        PathBuf::from(name)
    }
}

impl Write for State {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "日志文件已关闭"))?;
        let n = file.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

/// make_writer 返回的写入句柄（std 未为 MutexGuard 实现 io::Write，包一层）。
pub struct StateWriter<'a>(MutexGuard<'a, State>);

impl Write for StateWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a> MakeWriter<'a> for RollingFile {
    type Writer = StateWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        let mut guard = self.lock_state();
        guard.rotate_if_needed();
        StateWriter(guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("p2p-log-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_chunk(rolling: &RollingFile, bytes: &[u8]) {
        let mut w = rolling.make_writer();
        w.write_all(bytes).unwrap();
        w.flush().unwrap();
    }

    #[test]
    fn rotates_when_size_cap_reached() {
        let dir = temp_dir("rotate");
        let base = dir.join("app.log");
        let rolling = RollingFile::new(base.clone(), 16, 3).unwrap();
        write_chunk(&rolling, b"aaaaaaaa"); // 8 字节
        write_chunk(&rolling, b"bbbbbbbb"); // 16 字节
        assert!(!base.with_file_name("app.log.1").exists(), "未到上限不滚动");
        write_chunk(&rolling, b"cccccccc"); // 触发滚动后写入
        assert_eq!(
            std::fs::read(base.with_file_name("app.log.1")).unwrap(),
            b"aaaaaaaabbbbbbbb"
        );
        assert_eq!(std::fs::read(&base).unwrap(), b"cccccccc");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn archives_capped_and_oldest_dropped() {
        let dir = temp_dir("cap");
        let base = dir.join("app.log");
        let rolling = RollingFile::new(base.clone(), 4, 2).unwrap();
        for i in 0u8..8 {
            write_chunk(&rolling, &[b'a' + i; 4]);
        }
        assert!(base.exists());
        assert!(base.with_file_name("app.log.1").exists());
        assert!(base.with_file_name("app.log.2").exists());
        assert!(!base.with_file_name("app.log.3").exists(), "归档份数必须封顶");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reopen_appends_and_counts_existing_size() {
        let dir = temp_dir("reopen");
        let base = dir.join("app.log");
        {
            let rolling = RollingFile::new(base.clone(), 16, 2).unwrap();
            write_chunk(&rolling, b"0123456789ABCDEF");
        }
        let rolling = RollingFile::new(base.clone(), 16, 2).unwrap();
        write_chunk(&rolling, b"zzzz");
        assert!(
            base.with_file_name("app.log.1").exists(),
            "重开沿用已有大小：写满即滚动，不无限追加"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_zero_caps() {
        let dir = temp_dir("zerocap");
        let err = RollingFile::new(dir.join("app.log"), 0, 1).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        let err = RollingFile::new(dir.join("app.log"), 16, 0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
