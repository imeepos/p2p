//! 邀请制加好友门面（Chat 公共 API）：邀请 / 列表 / 同意 / 拒绝 / 撤回。
//! 语义：添加必须经对方同意，同意后双向互为好友；直加仅供测试引导（friend_add）。

use std::sync::Arc;

use crate::friend::ChatFriend;
use crate::invite::{FriendInvite, InviteDirection};
use crate::model::{now_ms, parse_peer_id, validate_nickname, ChatError};
use crate::wire_invite::{deliver_frame, InviteFrame, ACCEPT, INVITE, REJECT};
use crate::ChatCore;

/// 邀请结果：delivered = 本次已送达对端（false = 对端离线，挂起待重连重投）。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteReport {
    pub invite: FriendInvite,
    pub delivered: bool,
}

impl crate::Chat {
    /// 发邀请：校验后登记本机待同意邀请（out）并尽力投递；重复邀请幂等刷新。
    /// 对方同意前好友簿不变；对端离线保持挂起，重连时由 outbox 任务重投。
    pub async fn friend_invite(
        &self,
        peer_id: &str,
        nickname: &str,
        addrs: Vec<String>,
        note: Option<String>,
    ) -> Result<InviteReport, ChatError> {
        let peer = parse_peer_id(peer_id)?;
        if peer == self.core.node.local_peer_id() {
            return Err(ChatError::SelfPeer(peer_id.to_string()));
        }
        if self
            .core
            .store
            .friends_list()?
            .iter()
            .any(|f| f.peer_id == peer_id)
        {
            return Err(ChatError::AlreadyFriends(peer_id.to_string()));
        }
        let nickname = validate_nickname(nickname)?;
        for addr in &addrs {
            self.core
                .node
                .add_peer_address(peer, addr)
                .map_err(|e| ChatError::InvalidAddr(format!("{addr}: {e}")))?;
        }
        let invite = FriendInvite {
            peer_id: peer_id.to_string(),
            nickname,
            addrs,
            note,
            direction: InviteDirection::Out,
            ts_ms: now_ms(),
            delivered: false,
        };
        self.core.store.upsert_invite(invite.clone())?;
        let delivered = self.deliver_invite(&invite).await;
        Ok(InviteReport { invite, delivered })
    }

    /// 邀请列表（out 待对方同意 + in 待本机处理，落盘序）。
    pub fn invites_list(&self) -> Result<Vec<FriendInvite>, ChatError> {
        Ok(self.core.store.invites_list()?)
    }

    /// 同意来邀：本机立即建好友（互为好友的本侧），并回投 ACCEPT 完成对端建簿。
    /// nickname 空串 = 沿用邀请内对端自称。
    pub async fn invite_accept(
        &self,
        peer_id: &str,
        nickname: &str,
    ) -> Result<ChatFriend, ChatError> {
        parse_peer_id(peer_id)?;
        let invite = self
            .core
            .store
            .invites_list()?
            .into_iter()
            .find(|i| i.peer_id == peer_id && i.direction == InviteDirection::In)
            .ok_or_else(|| ChatError::NotFound(format!("无待处理邀请：{peer_id}")))?;
        let name = {
            let t = validate_nickname(nickname)?;
            if t.is_empty() {
                invite.nickname.clone()
            } else {
                t
            }
        };
        let friend = insert_friend(
            &self.core,
            peer_id,
            &name,
            invite.addrs.clone(),
            invite.note.clone(),
        )?;
        self.core
            .store
            .remove_invite(peer_id, InviteDirection::In)?;
        self.reply_frame(peer_id, ACCEPT, "", Vec::new()).await;
        Ok(friend)
    }

    /// 拒绝来邀：移除本机 in 邀请并通知对方（通知尽力而为）。
    pub async fn invite_reject(&self, peer_id: &str) -> Result<(), ChatError> {
        parse_peer_id(peer_id)?;
        let removed = self
            .core
            .store
            .remove_invite(peer_id, InviteDirection::In)?;
        if !removed {
            return Err(ChatError::NotFound(format!("无待处理邀请：{peer_id}")));
        }
        self.reply_frame(peer_id, REJECT, "", Vec::new()).await;
        Ok(())
    }

    /// 撤回本机待同意邀请（对方未必在线，通知尽力而为）。
    pub async fn invite_cancel(&self, peer_id: &str) -> Result<bool, ChatError> {
        parse_peer_id(peer_id)?;
        let removed = self
            .core
            .store
            .remove_invite(peer_id, InviteDirection::Out)?;
        if removed {
            self.reply_frame(peer_id, REJECT, "", Vec::new()).await;
        }
        Ok(removed)
    }

    /// 单帧尽力投递；失败保持挂起（可观测 warn），由重连重投收敛。
    /// 帧携带本机 listen_addrs：对端同意后可凭此回拨（可拨性自举）。
    async fn deliver_invite(&self, invite: &FriendInvite) -> bool {
        let addrs = frame_addrs(&self.core);
        let local = self.core.node.local_peer_id();
        let frame = InviteFrame::new(&local, &invite.nickname, addrs);
        match deliver_frame(&self.core, &invite.peer_id, INVITE, &frame, false).await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(peer = %invite.peer_id, error = %e, "邀请投递失败，挂起待重连重投");
                false
            }
        }
    }

    /// 同意/拒绝回投：失败仅告警——对端重投 INVITE 时按已是好友/无邀请语义幂等收敛。
    async fn reply_frame(&self, peer_id: &str, kind: u8, nickname: &str, addrs: Vec<String>) {
        let local = self.core.node.local_peer_id();
        let frame = InviteFrame::new(&local, nickname, addrs);
        if let Err(e) = deliver_frame(&self.core, peer_id, kind, &frame, false).await {
            tracing::warn!(peer = %peer_id, kind, error = %e, "邀请回投失败，等待对端重连收敛");
        }
    }
}

/// 启动自愈：本机地址随重启变化，挂起邀请的对端仍持旧地址——主动重投
/// （无视 delivered 标记）让对端经 INVITE 帧学习新地址并收敛双向建簿。
pub(crate) fn spawn_invite_heal(core: Arc<ChatCore>) {
    tokio::spawn(async move {
        // 等监听与 outbox 任务就绪再重投，避免启动竞态。
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let peers: Vec<String> = match core.store.invites_list() {
            Ok(list) => list
                .into_iter()
                .filter(|i| i.direction == InviteDirection::Out)
                .map(|i| i.peer_id)
                .collect::<Vec<_>>(),
            Err(e) => {
                tracing::warn!(error = %e, "启动自愈读取邀请簿失败");
                return;
            }
        };
        for peer in peers {
            // 拨号就绪时序不稳（底座启动竞态），退避重试三次尽力收敛。
            for attempt in 0..5u32 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
                match deliver_frame(&core, &peer, INVITE, &invite_frame(&core, &peer), false).await
                {
                    Ok(()) => break,
                    Err(e) => {
                        tracing::warn!(peer = %peer, attempt, error = %e, "启动自愈重投失败");
                    }
                }
            }
        }
    });
}

/// INVITE 帧地址来源：声明地址（serve 发布）优先，缺省回退当前监听地址。
pub(crate) fn frame_addrs(core: &ChatCore) -> Vec<String> {
    let advertised = core.store.advertised_load();
    if advertised.is_empty() {
        core.node.listen_addrs()
    } else {
        advertised
    }
}

/// 构造携带本机最新地址的 INVITE 帧。
fn invite_frame(core: &ChatCore, peer: &str) -> InviteFrame {
    let local = core.node.local_peer_id();
    let nickname = core
        .store
        .invites_list()
        .unwrap_or_default()
        .into_iter()
        .find(|i| i.peer_id == peer && i.direction == InviteDirection::Out)
        .map(|i| i.nickname)
        .unwrap_or_default();
    InviteFrame::new(&local, &nickname, core.node.listen_addrs())
}

/// PeerConnected 时重投该 peer 的挂起邀请（outbox 任务联动）。
pub(crate) async fn flush_invites_peer(core: &ChatCore, peer: &str) {
    let pending: Vec<FriendInvite> = core
        .store
        .invites_list()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| i.peer_id == peer && i.direction == InviteDirection::Out)
        .collect();
    for invite in pending {
        let local = core.node.local_peer_id();
        let frame = InviteFrame::new(&local, &invite.nickname, frame_addrs(core));
        if let Err(e) = deliver_frame(core, peer, INVITE, &frame, true).await {
            tracing::warn!(peer = %peer, error = %e, "挂起邀请重投失败，保持挂起");
        }
    }
}

/// 重启后把挂起邀请里的对端地址回填节点地址簿（同意路径可拨性持久化）。
pub(crate) fn rearm_invite_addrs(core: &ChatCore) -> Result<(), ChatError> {
    for invite in core.store.invites_list()? {
        let peer = match parse_peer_id(&invite.peer_id) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(peer_id = %invite.peer_id, error = %e, "邀请 peerId 非法，跳过地址登记");
                continue;
            }
        };
        for addr in &invite.addrs {
            if let Err(e) = core.node.add_peer_address(peer, addr) {
                tracing::warn!(peer_id = %invite.peer_id, addr = %addr, error = %e, "邀请地址登记失败");
            }
        }
    }
    Ok(())
}

/// 建好友公共路径（同意/互邀自愈共用）：登记地址簿 + 原子写好友簿。
pub(crate) fn insert_friend(
    core: &Arc<ChatCore>,
    peer_id: &str,
    nickname: &str,
    addrs: Vec<String>,
    note: Option<String>,
) -> Result<ChatFriend, ChatError> {
    let peer = parse_peer_id(peer_id)?;
    let nickname = validate_nickname(nickname)?;
    for addr in &addrs {
        if let Err(e) = core.node.add_peer_address(peer, addr) {
            tracing::warn!(peer_id = %peer_id, addr = %addr, error = %e, "邀请好友地址登记失败");
        }
    }
    let friend = ChatFriend {
        peer_id: peer_id.to_string(),
        nickname,
        addrs,
        note,
        group: None,
    };
    core.store.upsert_friend(friend.clone())?;
    Ok(friend)
}
