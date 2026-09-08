//! Chat 装配（构造块行数红线拆分自 lib.rs）：存储/事件通道/三协议 handler/
//! 群挂载/outbox 与邀请自愈任务，GUI 与 CLI 共用同一装配路径。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::Node;
use tokio::sync::broadcast;

use crate::core::ChatCore;
use crate::group::Group;
use crate::invite_api;
use crate::invite_handler;
use crate::outbox;
use crate::profile_wire;
use crate::wire;
use crate::{Chat, ChatError, LocalProfileFn, CHAT_PROTOCOL, INVITE_PROTOCOL, PROFILE_PROTOCOL};

const EVENT_CAPACITY: usize = 128; // 1:1 与群各自独立事件通道

impl Chat {
    pub fn new(node: Arc<Node>, data_dir: PathBuf) -> Result<Self, ChatError> {
        Self::with_local_profile(node, data_dir, Arc::new(crate::PeerProfile::default))
    }

    /// 带本机资料供给的装配：GUI 接 NodeProfile 持久层，/im/profile/1
    /// 应答据此返回；其余装配路径与 Chat::new 完全一致。
    pub fn with_local_profile(
        node: Arc<Node>,
        data_dir: PathBuf,
        local_profile: LocalProfileFn,
    ) -> Result<Self, ChatError> {
        let store = crate::store::Store::new(data_dir.join("chat"))?;
        let (tx, _) = broadcast::channel(EVENT_CAPACITY);
        let core = Arc::new(ChatCore {
            node,
            store,
            events: tx.clone(),
            send_locks: std::sync::Mutex::new(std::collections::HashMap::new()),
            flush_tried: std::sync::Mutex::new(std::collections::HashMap::new()),
            local_profile,
        });
        core.rearm_friend_addrs()?;
        invite_api::rearm_invite_addrs(&core)?;
        register(core.clone(), CHAT_PROTOCOL, |core, proto| {
            Arc::new(wire::ChatHandler::new(core, proto))
        })?;
        register(core.clone(), INVITE_PROTOCOL, |core, proto| {
            Arc::new(invite_handler::InviteHandler::new(core, proto))
        })?;
        register(core.clone(), PROFILE_PROTOCOL, |core, proto| {
            Arc::new(profile_wire::ProfileHandler::new(core, proto))
        })?;
        let group = Group::mount(core.clone(), &data_dir)?;
        outbox::spawn_outbox_task(core.clone(), group.core.clone());
        outbox::spawn_outbox_sweeper(core.clone());
        invite_api::spawn_invite_heal(core.clone());
        Ok(Self { core, group })
    }
}

/// 协议 handler 注册：ProtocolId 构造失败映射 ChatError（与旧内联写法同语义）。
fn register(
    core: Arc<ChatCore>,
    protocol: &str,
    handler: impl FnOnce(Arc<ChatCore>, p2p::ProtocolId) -> Arc<dyn p2p::ProtocolHandler>,
) -> Result<(), ChatError> {
    let proto = p2p::ProtocolId::new(protocol).map_err(|e| ChatError::Protocol(e.to_string()))?;
    let handler = handler(core.clone(), proto);
    core.node.handle_protocol(handler);
    Ok(())
}
