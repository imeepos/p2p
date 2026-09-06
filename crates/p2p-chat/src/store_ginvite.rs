//! 入群邀请簿存储（group_invites.json）：锁内「重读磁盘 → 合并 → 原子写」纪律
//! 同 invites.json（store_invite.rs）；impl Store 块寄生于 store.rs 的类型定义。

use std::path::PathBuf;

use crate::ginvite::{GroupInvite, MAX_GROUP_INVITES};
use crate::store::Store;

/// 数组读取：损坏或缺失回退空簿并留 warn（同 invites 纪律，不静默）。
fn load_ginvites(path: &std::path::Path) -> Vec<GroupInvite> {
    crate::store_io::load_json_file(path, "group_invites.json")
}

impl Store {
    pub(crate) fn group_invites_lock_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.lock", self.group_invites_path.display()))
    }

    /// 全量邀请列表（in + out 混排，落盘序；展示排序由门面按 tsMs 倒序）。
    pub(crate) fn group_invites_list(&self) -> Result<Vec<GroupInvite>, std::io::Error> {
        Ok(load_ginvites(&self.group_invites_path))
    }

    /// upsert 邀请：同方向+同群+同对端视为同一条（刷新全部字段），不新增条目。
    pub(crate) fn upsert_group_invite(&self, invite: GroupInvite) -> Result<(), std::io::Error> {
        let _lock = crate::store_invite::lock_exclusive(self, self.group_invites_lock_path())?;
        let mut list = load_ginvites(&self.group_invites_path);
        match list.iter_mut().find(|i| i.same_slot(&invite)) {
            Some(slot) => *slot = invite,
            None => {
                if list.len() >= MAX_GROUP_INVITES {
                    return Err(std::io::Error::other(format!(
                        "入群邀请簿已满（上限 {MAX_GROUP_INVITES}），请先清理过期邀请"
                    )));
                }
                list.push(invite);
            }
        }
        let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
        crate::store_io::atomic_write(&self.group_invites_path, &bytes)
    }

    /// 按 id 查条目。
    pub(crate) fn find_group_invite(
        &self,
        id: &str,
    ) -> Result<Option<GroupInvite>, std::io::Error> {
        Ok(self.group_invites_list()?.into_iter().find(|i| i.id == id))
    }

    /// 按 id 原位修补（状态迁移 / delivered 标记）；返回修补后条目，未命中 None。
    pub(crate) fn patch_group_invite(
        &self,
        id: &str,
        f: impl FnOnce(&mut GroupInvite),
    ) -> Result<Option<GroupInvite>, std::io::Error> {
        let hit = {
            let _lock = crate::store_invite::lock_exclusive(self, self.group_invites_lock_path())?;
            let mut list = load_ginvites(&self.group_invites_path);
            let mut hit = None;
            if let Some(invite) = list.iter_mut().find(|i| i.id == id) {
                f(invite);
                hit = Some(invite.clone());
            }
            if hit.is_some() {
                let bytes = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
                crate::store_io::atomic_write(&self.group_invites_path, &bytes)?;
            }
            hit
        };
        Ok(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ginvite::{GroupInviteDirection, GroupInviteState};

    fn temp_store(tag: &str) -> (Store, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("p2p-ginvite-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        (Store::new(dir.clone()).expect("store"), dir)
    }

    fn invite(id: &str, group: &str, direction: GroupInviteDirection) -> GroupInvite {
        GroupInvite {
            id: id.into(),
            group_id: group.into(),
            group_name: "g".into(),
            owner: "o".into(),
            inviter: "o".into(),
            invitee: "e".into(),
            note: None,
            direction,
            state: GroupInviteState::Pending,
            ts_ms: 7,
            delivered: false,
        }
    }

    #[test]
    fn upsert_same_slot_replaces_and_keeps_distinct_slots() {
        let (store, dir) = temp_store("upsert");
        store
            .upsert_group_invite(invite("i1", "g1", GroupInviteDirection::Out))
            .expect("insert");
        let mut refreshed = invite("i9", "g1", GroupInviteDirection::Out);
        refreshed.state = GroupInviteState::Rejected;
        store.upsert_group_invite(refreshed).expect("refresh");
        let list = store.group_invites_list().expect("list");
        assert_eq!(list.len(), 1, "同槽位刷新不新增");
        assert_eq!(list[0].id, "i9", "条目整体替换");
        assert_eq!(list[0].state, GroupInviteState::Rejected);
        store
            .upsert_group_invite(invite("i2", "g1", GroupInviteDirection::In))
            .expect("opposite direction is distinct");
        assert_eq!(store.group_invites_list().expect("list").len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cap_rejects_entry_beyond_limit() {
        let (store, dir) = temp_store("cap");
        for n in 0..MAX_GROUP_INVITES {
            store
                .upsert_group_invite(invite(
                    &format!("i{n}"),
                    &format!("g{n}"),
                    GroupInviteDirection::Out,
                ))
                .expect("insert within cap");
        }
        let err = store
            .upsert_group_invite(invite("overflow", "gx", GroupInviteDirection::Out))
            .expect_err("超限必须显式 Err");
        assert!(err.to_string().contains("已满"), "err: {err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn state_machine_lifecycle_through_patch_and_refresh() {
        let (store, dir) = temp_store("lifecycle");
        store
            .upsert_group_invite(invite("i1", "g1", GroupInviteDirection::In))
            .expect("pending 建条");
        // pending → accepted（roster 收敛迁移）
        store
            .patch_group_invite("i1", |i| {
                i.state = GroupInviteState::Accepted;
                i.delivered = true;
            })
            .expect("patch");
        assert_eq!(
            store
                .find_group_invite("i1")
                .expect("find")
                .map(|i| i.state),
            Some(GroupInviteState::Accepted)
        );
        // 重复邀请 upsert 刷新：accepted → pending、delivered 复位（重新决策窗口）；
        // 门面 refresh_or_create 复用既有 id，此处同构构造。
        store
            .upsert_group_invite(invite("i1", "g1", GroupInviteDirection::In))
            .expect("refresh");
        let refreshed = store.find_group_invite("i1").expect("find").expect("存在");
        assert_eq!(refreshed.state, GroupInviteState::Pending, "刷新回 pending");
        assert!(!refreshed.delivered, "delivered 复位");
        assert_eq!(refreshed.id, "i1", "条目 id 稳定");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn patch_by_id_reports_hit_and_persists() {
        let (store, dir) = temp_store("patch");
        store
            .upsert_group_invite(invite("i1", "g1", GroupInviteDirection::In))
            .expect("insert");
        let patched = store
            .patch_group_invite("i1", |i| {
                i.state = GroupInviteState::Accepted;
                i.delivered = true;
            })
            .expect("patch")
            .expect("命中返回条目");
        assert_eq!(patched.state, GroupInviteState::Accepted);
        assert!(patched.delivered);
        let reread = store.find_group_invite("i1").expect("find").expect("存在");
        assert_eq!(reread, patched, "落盘可读回");
        assert!(store
            .patch_group_invite("missing", |_| {})
            .expect("miss")
            .is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
