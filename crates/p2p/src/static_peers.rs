//! 静态对端登记的持久化原语（社交化发现 P1，social-discovery-plan.md §5）：
//! JSON 落盘（0600、tmp+rename 原子替换），启动载入 AddressBook（Manual
//! 来源）；好友/联系人等业务语义由业务层在此之上构建。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

/// 单条静态登记：PeerId（base58）+ 可拨地址 + 业务备注（底座不解释）。
/// W2b/CC4 起对 src-tauri 开放（GUI 静态对端簿命令面复用本持久化原语）。
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct StaticPeerEntry {
    pub peer_id: String,
    pub addrs: Vec<String>,
    #[serde(default)]
    pub note: String,
}

/// 文件句柄：路径 + 进程内副本。upsert 即改副本并整文件重写。
pub struct StaticPeersFile {
    path: PathBuf,
    entries: Mutex<Vec<StaticPeerEntry>>,
}

impl StaticPeersFile {
    /// 载入已有文件（不存在 = 空册）；坏内容按 io::InvalidData 上抛，
    /// 由装配方决定告警或拒绝启动。
    pub fn load(path: PathBuf) -> io::Result<Self> {
        let entries = load_entries(&path)?;
        Ok(Self {
            path,
            entries: Mutex::new(entries),
        })
    }

    /// 按 peer_id 覆盖登记并落盘（0600、tmp+rename）。
    pub fn upsert(
        &self,
        peer_id: String,
        addrs: Vec<String>,
        note: String,
    ) -> io::Result<()> {
        let mut entries = Self::lock(&self.entries);
        entries.retain(|e| e.peer_id != peer_id);
        entries.push(StaticPeerEntry {
            peer_id,
            addrs,
            note,
        });
        save(&self.path, &entries)
    }

    /// 按 peer_id 移除登记并落盘；不存在 = 无变化成功返回（幂等）。
    pub fn remove(&self, peer_id: &str) -> io::Result<()> {
        let mut entries = Self::lock(&self.entries);
        entries.retain(|e| e.peer_id != peer_id);
        save(&self.path, &entries)
    }

    pub fn entries(&self) -> Vec<StaticPeerEntry> {
        Self::lock(&self.entries).clone()
    }

    fn lock(m: &Mutex<Vec<StaticPeerEntry>>) -> MutexGuard<'_, Vec<StaticPeerEntry>> {
        m.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn load_entries(path: &Path) -> io::Result<Vec<StaticPeerEntry>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

fn save(path: &Path, entries: &[StaticPeerEntry]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(entries)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip_preserves_entries() {
        let dir = std::env::temp_dir().join(format!("p2p-sp-{}", std::process::id()));
        let path = dir.join("static-peers.json");
        let file = StaticPeersFile::load(path.clone()).expect("load missing = empty");
        file.upsert(
            "peer-a".into(),
            vec!["10.0.0.1/u4000".into()],
            "node-a".into(),
        )
        .expect("upsert ok");
        file.upsert("peer-b".into(), vec!["10.0.0.2/u4001".into()], "".into())
            .expect("upsert ok");
        file.upsert(
            "peer-a".into(),
            vec!["10.0.0.9/u4999".into()],
            "moved".into(),
        )
        .expect("upsert overwrites");

        let reloaded = StaticPeersFile::load(path.clone()).expect("reload ok");
        let entries = reloaded.entries();
        assert_eq!(entries.len(), 2, "同 peer_id 覆盖不重复");
        let a = entries.iter().find(|e| e.peer_id == "peer-a").expect("a");
        assert_eq!(a.note, "moved");
        assert_eq!(a.addrs, vec!["10.0.0.9/u4999".to_string()]);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "落盘权限必须 0600");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_missing_file_is_empty_registry() {
        let path = std::env::temp_dir().join(format!("p2p-sp-miss-{}", std::process::id()));
        let file = StaticPeersFile::load(path).expect("missing = empty");
        assert!(file.entries().is_empty());
    }

    /// W2b/CC4：remove 语义（GUI static_peers_remove 命令面复用）——存在即删，
    /// 不存在亦 Ok（幂等）。
    #[test]
    fn remove_existing_and_missing_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("p2p-sp-rm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir ok");
        let path = dir.join("static-peers.json");
        let file = StaticPeersFile::load(path.clone()).expect("load ok");
        file.upsert("peer-a".into(), vec![], "".into()).expect("upsert ok");
        file.upsert("peer-b".into(), vec![], "".into()).expect("upsert ok");
        file.remove("peer-a").expect("remove existing ok");
        assert_eq!(file.entries().len(), 1, "仅剩 peer-b");
        file.remove("peer-a").expect("remove missing 亦 Ok（幂等）");
        assert_eq!(file.entries().len(), 1, "重复移除无副作用");

        let reloaded = StaticPeersFile::load(path).expect("reload ok");
        assert_eq!(
            reloaded.entries().iter().map(|e| e.peer_id.as_str()).collect::<Vec<_>>(),
            vec!["peer-b"],
            "移除落盘生效"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_corrupt_file_is_invalid_data_error() {
        let dir = std::env::temp_dir().join(format!("p2p-sp-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("dir ok");
        let path = dir.join("static-peers.json");
        fs::write(&path, b"{not json").expect("write ok");
        let err = match StaticPeersFile::load(path) {
            Err(e) => e,
            Ok(_) => panic!("corrupt file must be rejected"),
        };
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(dir);
    }
}
