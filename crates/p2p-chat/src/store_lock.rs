//! 好友簿跨进程写锁（R1 防静默丢写）：持有期覆盖「锁内重读磁盘 → 合并 → 原子写」。
//! Unix 用 flock（LOCK_EX|LOCK_NB 自旋 + 截止时间；进程崩溃内核自动释放，无陈锁）；
//! 其他平台退化为 O_EXCL 独占创建自旋（崩溃残留陈锁时后续写显式超时报错，不静默）。
//! 超时错误携带锁路径与等待时长，失败路径一律可观测。

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

/// 自旋重试间隔。
const RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// friends.json 写锁句柄；Drop 显式释放并留告警，进程崩溃由 OS 兜底。
#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct FileLock {
    file: fs::File,
}

/// 非 unix 退化为独占锁文件；Drop 负责清理，崩溃残留的陈锁令后续写显式超时。
#[cfg(not(unix))]
#[derive(Debug)]
pub(crate) struct FileLock {
    path: PathBuf,
}

impl FileLock {
    /// 排他获取；timeout 内未得则显式报错（含锁路径），绝不静默降级为无锁写。
    pub(crate) fn acquire(path: PathBuf, timeout: Duration) -> io::Result<Self> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(lock) = try_acquire_once(&path)? {
                return Ok(lock);
            }
            if Instant::now() >= deadline {
                return Err(lock_timeout_error(&path, timeout));
            }
            std::thread::sleep(RETRY_INTERVAL);
        }
    }
}

#[cfg(unix)]
fn try_acquire_once(path: &Path) -> io::Result<Option<FileLock>> {
    use std::os::fd::AsRawFd;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(Some(FileLock { file }));
    }
    let err = io::Error::last_os_error();
    if err.kind() == io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(err)
    }
}

#[cfg(not(unix))]
fn try_acquire_once(path: &Path) -> io::Result<Option<FileLock>> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(_) => Ok(Some(FileLock {
            path: path.to_path_buf(),
        })),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let rc = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        if rc != 0 {
            tracing::warn!(
                error = %io::Error::last_os_error(),
                "flock 显式释放失败（fd 关闭时内核仍会兜底释放）"
            );
        }
    }
}

#[cfg(not(unix))]
impl Drop for FileLock {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_file(&self.path) {
            tracing::warn!(
                path = %self.path.display(),
                error = %e,
                "锁文件删除失败，残留陈锁将令后续写显式超时"
            );
        }
    }
}

fn lock_timeout_error(path: &Path, timeout: Duration) -> io::Error {
    io::Error::other(format!(
        "好友簿写锁 {} 等待 {:?} 未获取：并发写者僵持或残留陈锁，拒绝静默覆盖",
        path.display(),
        timeout
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChatFriend;
    use crate::store::Store;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("p2p-chat-lock-{tag}-{}", std::process::id()))
    }

    #[test]
    fn held_lock_makes_second_writer_fail_explicitly() {
        let path = temp_path("timeout");
        let _guard =
            FileLock::acquire(path.clone(), Duration::from_millis(50)).expect("首个写者应立即获锁");
        let err = FileLock::acquire(path.clone(), Duration::from_millis(80))
            .expect_err("持锁期间第二写者必须显式失败而非静默并行");
        assert!(
            err.to_string().contains("拒绝静默覆盖"),
            "超时报错须含拒绝语义与锁路径: {err}"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn lock_is_reacquirable_after_release() {
        let path = temp_path("release");
        drop(FileLock::acquire(path.clone(), Duration::from_millis(50)).expect("首次获锁"));
        FileLock::acquire(path.clone(), Duration::from_millis(50)).expect("释放后应可重新获锁");
        let _ = fs::remove_file(&path);
    }

    fn friend(peer: &str) -> ChatFriend {
        ChatFriend {
            peer_id: peer.to_string(),
            nickname: "race".into(),
            addrs: Vec::new(),
            note: None,
        }
    }

    /// 回归：两个 Store（跨进程等价场景）先后加好友，磁盘必须双全而非 last-write-wins。
    #[test]
    fn concurrent_store_upserts_merge_without_loss() {
        let dir = std::env::temp_dir().join(format!("p2p-chat-race-{}", std::process::id()));
        let a = Store::new(dir.clone()).expect("store a");
        let b = Store::new(dir.clone()).expect("store b（启动时磁盘尚未有好友）");
        a.upsert_friend(friend("4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi"))
            .expect("a 加好友");
        b.upsert_friend(friend("8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR"))
            .expect("b 加好友");
        let book = crate::store_io::load_friends(&dir.join("friends.json"));
        assert_eq!(book.len(), 2, "并发写必须全量保留，实得: {book:?}");
        drop(a);
        drop(b);
        let _ = fs::remove_dir_all(dir);
    }
}
