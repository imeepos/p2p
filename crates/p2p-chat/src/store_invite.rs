//! 邀请簿存储（invites.json）：锁内「重读磁盘 → 合并 → 原子写」纪律同 friends.json。
//! 独立文件拆分（store.rs 行数红线）；impl Store 块寄生于 store.rs 的类型定义。

use crate::invite::{FriendInvite, InviteDirection, MAX_INVITES};
use crate::store::Store;

fn poisoned() -> std::io::Error {
    std::io::Error::other("store 内部锁中毒")
}

impl Store {
    /// 全量邀请列表（out + in 混排，落盘序）。
    pub(crate) fn invites_list(&self) -> Result<Vec<FriendInvite>, std::io::Error> {
        let state = self.state.lock().map_err(|_| poisoned())?;
        Ok(state.invites.clone())
    }

    /// upsert 邀请：同 peer 同方向视为同一条（刷新昵称/地址/备注/时间），不新增条目。
    pub(crate) fn upsert_invite(&self, invite: FriendInvite) -> Result<(), std::io::Error> {
        let list = {
            let _lock = lock_exclusive(self, self.invites_lock_path())?;
            let mut list = crate::store_io::load_invites(&self.invites_path);
            match list
                .iter_mut()
                .find(|i| i.peer_id == invite.peer_id && i.direction == invite.direction)
            {
                Some(slot) => *slot = invite,
                None => {
                    if list.len() >= MAX_INVITES {
                        return Err(std::io::Error::other(format!(
                            "邀请簿已满（上限 {MAX_INVITES}），请先清理过期邀请"
                        )));
                    }
                    list.push(invite);
                }
            }
            let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
            crate::store_io::atomic_write(&self.invites_path, &bytes)?;
            list
        };
        self.sync_invites_memory(list)
    }

    /// 标记邀请已送达（锁内落盘 + 回灌内存）；INVITE 帧 ACK 成功后调用。
    pub(crate) fn mark_invite_delivered(
        &self,
        peer_id: &str,
        direction: InviteDirection,
    ) -> Result<(), std::io::Error> {
        let list = {
            let _lock = lock_exclusive(self, self.invites_lock_path())?;
            let mut list = crate::store_io::load_invites(&self.invites_path);
            for i in list.iter_mut() {
                if i.peer_id == peer_id && i.direction == direction {
                    i.delivered = true;
                }
            }
            let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
            crate::store_io::atomic_write(&self.invites_path, &bytes)?;
            list
        };
        self.sync_invites_memory(list)
    }

    /// 删除指定 peer 指定方向的邀请；返回是否真的删了。
    pub(crate) fn remove_invite(
        &self,
        peer_id: &str,
        direction: InviteDirection,
    ) -> Result<bool, std::io::Error> {
        let removed = {
            let _lock = lock_exclusive(self, self.invites_lock_path())?;
            let disk = crate::store_io::load_invites(&self.invites_path);
            let next: Vec<FriendInvite> = disk
                .iter()
                .filter(|i| !(i.peer_id == peer_id && i.direction == direction))
                .cloned()
                .collect();
            let hit = next.len() != disk.len();
            if hit {
                let bytes = serde_json::to_vec_pretty(&next).map_err(std::io::Error::other)?;
                crate::store_io::atomic_write(&self.invites_path, &bytes)?;
            }
            self.sync_invites_memory(next)?;
            hit
        };
        Ok(removed)
    }

    fn sync_invites_memory(&self, list: Vec<FriendInvite>) -> Result<(), std::io::Error> {
        let mut state = self.state.lock().map_err(|_| poisoned())?;
        state.invites = list;
        Ok(())
    }
}

/// FileLock::acquire 的短别名（锁超时取 store 统一配置）。
fn lock_exclusive(
    store: &Store,
    path: std::path::PathBuf,
) -> Result<crate::store_lock::FileLock, std::io::Error> {
    crate::store_lock::FileLock::acquire(path, store.lock_timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> (Store, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("p2p-invite-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        (Store::new(dir.clone()).expect("store"), dir)
    }

    fn invite(peer: &str, direction: InviteDirection) -> FriendInvite {
        FriendInvite {
            peer_id: peer.into(),
            nickname: "n".into(),
            addrs: vec![],
            note: None,
            direction,
            ts_ms: 7,
            delivered: false,
        }
    }

    #[test]
    fn upsert_same_peer_same_direction_replaces_entry() {
        let (store, dir) = temp_store("upsert");
        store
            .upsert_invite(invite("p", InviteDirection::In))
            .expect("insert");
        let mut refreshed = invite("p", InviteDirection::In);
        refreshed.nickname = "n2".into();
        store.upsert_invite(refreshed).expect("refresh");
        assert_eq!(store.invites_list().expect("list").len(), 1, "同条目不重复");
        let mut other = invite("p", InviteDirection::Out);
        other.nickname = "out".into();
        store
            .upsert_invite(other)
            .expect("opposite direction is distinct");
        assert_eq!(store.invites_list().expect("list").len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn remove_is_direction_scoped_and_reports_hit() {
        let (store, dir) = temp_store("remove");
        store
            .upsert_invite(invite("p", InviteDirection::In))
            .expect("insert");
        assert!(
            !store
                .remove_invite("p", InviteDirection::Out)
                .expect("remove out"),
            "方向不匹配不删"
        );
        assert!(
            store
                .remove_invite("p", InviteDirection::In)
                .expect("remove in"),
            "命中返回 true"
        );
        assert!(store.invites_list().expect("list").is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
