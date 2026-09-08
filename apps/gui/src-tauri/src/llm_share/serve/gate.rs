//! AllowlistGate（llm-share-link 设计 §5.3，W3）：出借方 allowlist 共享句柄。
//! 文件（<data-dir>/llm-share/allowlist.json）为持久事实源，内存为 admit 判定
//! 缓存；写入统一走 p2p-cli allowlist（同一条 tmp+rename 原子写），随后内存
//! 回读同源。admit 按条目存在且未过期判定（到期经 admit 惰性判定，过期条目
//! 拒绝）；跨进程 CLI 直写文件经 mtime 感知在下次 admit 前重读。兑换激活的
//! 互斥临界区持本门禁写锁（allowlist 写与台账激活同域串行）。

use std::collections::BTreeMap;
use std::sync::RwLock;

use p2p_cli::llm_share::allowlist::{self, AllowEntry, AllowReport, DenyReport};

/// 门禁状态：内存条目表 + 上次读盘的文件 mtime（跨进程变更感知）。
pub struct GateState {
    pub(crate) entries: BTreeMap<String, AllowEntry>,
    pub(crate) file_mtime: Option<std::time::SystemTime>,
}

pub struct AllowlistGate {
    data_dir: String,
    lock: RwLock<GateState>,
}

impl AllowlistGate {
    /// 装配期加载：allowlist.json 损坏=显式 Err（不静默回空表，默认拒绝语义下
    /// 静默回空会把已有授权翻转成全拒）。
    pub fn load(data_dir: &str) -> Result<Self, String> {
        let state = Self::read_disk(data_dir)?;
        Ok(Self {
            data_dir: data_dir.to_owned(),
            lock: RwLock::new(state),
        })
    }

    fn read_disk(data_dir: &str) -> Result<GateState, String> {
        let file = allowlist::path(data_dir);
        let entries = allowlist::load_or_empty(&file)?.entries;
        let file_mtime = std::fs::metadata(&file)
            .ok()
            .and_then(|m| m.modified().ok());
        Ok(GateState {
            entries,
            file_mtime,
        })
    }

    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }

    /// admit 判定：条目存在且未过期；mtime 变化先回读磁盘（跨进程 CLI 写同源）。
    pub fn admits(&self, peer_id: &str, now: u64) -> bool {
        let mut state = self
            .lock
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::refresh_if_stale(&self.data_dir, &mut state);
        match state.entries.get(peer_id) {
            Some(entry) => !expired(entry, now),
            None => false,
        }
    }

    /// allow 主流程（serve 在装配时走同一写路径）：文件写 + 内存回读。
    pub fn allow(
        &self,
        peer_id: &str,
        models: &[String],
        note: Option<&str>,
        source: Option<&str>,
        expires_at: Option<u64>,
        granted_at: &str,
    ) -> Result<AllowReport, String> {
        let report = allowlist::allow(
            &self.data_dir,
            peer_id,
            models,
            note,
            source,
            expires_at,
            granted_at,
        )?;
        self.reload();
        Ok(report)
    }

    /// deny 主流程：不存在条目显式 Err（默认拒绝语义）。
    pub fn deny(&self, peer_id: &str) -> Result<DenyReport, String> {
        let report = allowlist::deny(&self.data_dir, peer_id)?;
        self.reload();
        Ok(report)
    }

    /// 按 source 级联删除（分享撤销），返回移除数；>0 才有磁盘变化。
    pub fn remove_by_source(&self, source: &str) -> Result<usize, String> {
        let removed = allowlist::remove_by_source(&self.data_dir, source)?;
        if removed > 0 {
            self.reload();
        }
        Ok(removed)
    }

    /// 外部（p2p-cli 共享写路径）直写后的显式回读（如 share_revoke 级联后）。
    pub fn refresh(&self) {
        self.reload();
    }

    /// 兑换激活互斥临界区：持写锁执行闭包（allowlist 写与台账激活同域），
    /// 闭包内不得再经本门禁公开方法（重入死锁）；结束后内存已与磁盘同源。
    pub(crate) fn critical<R>(&self, step: impl FnOnce(&str, &mut GateState) -> R) -> R {
        let mut state = self
            .lock
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        step(&self.data_dir, &mut state)
    }

    /// 临界区内落 allow 条目：走 p2p-cli 文件写，再回读内存同源。
    pub(crate) fn write_allow_locked(
        data_dir: &str,
        state: &mut GateState,
        peer_id: &str,
        models: &[String],
        source: &str,
        expires_at: u64,
        granted_at: &str,
    ) -> Result<AllowReport, String> {
        let report = allowlist::allow(
            data_dir,
            peer_id,
            models,
            None,
            Some(source),
            Some(expires_at),
            granted_at,
        )?;
        *state = Self::read_disk(data_dir)?;
        Ok(report)
    }

    fn reload(&self) {
        let mut state = self
            .lock
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::read_into(&self.data_dir, &mut state);
    }

    fn refresh_if_stale(data_dir: &str, state: &mut GateState) {
        let stale = std::fs::metadata(allowlist::path(data_dir))
            .ok()
            .and_then(|m| m.modified().ok())
            .zip(state.file_mtime)
            .map(|(current, known)| current != known)
            .unwrap_or(false);
        if stale {
            Self::read_into(data_dir, state);
        }
    }

    /// 回读磁盘入内存；读取失败保留上次内存态（可观测，不翻转授权面）。
    fn read_into(data_dir: &str, state: &mut GateState) {
        match Self::read_disk(data_dir) {
            Ok(fresh) => *state = fresh,
            Err(e) => tracing::warn!("allowlist 内存回读失败（保留上次内存态）: {e}"),
        }
    }
}

/// 到期判定：None=不过期；<=now=已过期（admit 惰性判定，过期即拒）。
fn expired(entry: &AllowEntry, now: u64) -> bool {
    entry.expires_at.map(|at| now >= at).unwrap_or(false)
}
