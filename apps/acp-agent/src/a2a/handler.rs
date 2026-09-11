//! /a2a/1 handler（a2a-over-p2p-design §5）：首帧嗅探分流——card 相
//! （请求-响应+订阅推送）与 task 相（JSON-RPC 2.0，1 task=1 流，§5.2）。
//! 可见性 fail-closed：远程仅 public + 授权清单内 private（§6）。

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a2a::{CardFrame, SignedCard};
use async_trait::async_trait;
use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::sync::mpsc;

use crate::a2a::agents::AgentStore;
use crate::a2a::invites::InviteStore;
use crate::a2a::task::TaskService;
use crate::audit::AuditSink;
use crate::config::AgentConfig;

/// 订阅推送通道容量：满即丢帧留 WARN（订阅端下次心跳/拉取自愈）。
const PUSH_CAP: usize = 16;
/// 心跳间隔 = 卡片 TTL/2（design §7.1 拍板 Q4）；订阅簿 2×TTL 无心跳才除名。
pub fn heartbeat_interval() -> Duration {
    Duration::from_secs(a2a::TTL_DEFAULT_SECS / 2)
}

/// 订阅登记表：peer 键 -> 推送通道（handler select 循环消费）。
#[derive(Default)]
pub struct Subscribers {
    senders: Mutex<HashMap<String, mpsc::Sender<CardFrame>>>,
}

impl Subscribers {
    pub fn new() -> Self {
        Self::default()
    }

    fn register(&self, peer: &str) -> mpsc::Receiver<CardFrame> {
        let (tx, rx) = mpsc::channel(PUSH_CAP);
        self.lock().insert(peer.to_owned(), tx);
        rx
    }

    /// 变更广播：满/断通道丢弃并摘除（下次 list/心跳自愈，不静默留 WARN）。
    pub fn notify(&self, frame: CardFrame) {
        let mut guard = self.lock();
        let dead: Vec<String> = guard
            .iter()
            .filter(|(_, tx)| {
                matches!(
                    tx.try_send(frame.clone()),
                    Err(mpsc::error::TrySendError::Full(_))
                )
            })
            .map(|(peer, _)| peer.clone())
            .collect();
        guard.retain(|peer, tx| {
            let closed = tx.is_closed();
            if closed {
                tracing::warn!(target: "a2a_audit", peer = %peer, "subscriber gone, pruned");
            }
            !closed
        });
        for peer in dead {
            tracing::warn!(target: "a2a_audit", peer = %peer, "subscriber push full, frame dropped");
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, mpsc::Sender<CardFrame>>> {
        self.senders
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// A2A 依赖面：簿 + 签名密钥 + 宿主 PeerId + 订阅表 + task 服务 + 审计。
pub struct A2aDeps {
    pub config: AgentConfig,
    pub agents: Arc<AgentStore>,
    pub invites: Arc<InviteStore>,
    pub keypair: p2p_identity::Keypair,
    pub host_peer: String,
    pub subscribers: Arc<Subscribers>,
    pub tasks: Arc<TaskService>,
    pub audit: Arc<dyn AuditSink>,
}

pub struct A2aHandler {
    deps: Arc<A2aDeps>,
    protocol_id: ProtocolId,
}

impl A2aHandler {
    pub fn new(deps: Arc<A2aDeps>) -> Result<Self, p2p_protocol::ProtocolError> {
        Ok(Self {
            deps,
            protocol_id: ProtocolId::new(a2a::PROTOCOL_ID)?,
        })
    }

    /// 对请求方可见的卡（§5.1 F4/F6）：owner 全见；远程 = public + 授权清单内。
    fn visible_cards(&self, peer: &PeerId, requester_is_owner: bool) -> Vec<SignedCard> {
        let now = unix_now();
        let granted: Vec<String> = if requester_is_owner {
            Vec::new()
        } else {
            self.deps.tasks.grants.agents_for_peer(&peer.to_string())
        };
        self.deps.agents.signed_cards_for(
            &self.deps.keypair,
            &self.deps.host_peer,
            requester_is_owner,
            &granted,
            now,
        )
    }
}

#[async_trait]
impl ProtocolHandler for A2aHandler {
    fn protocol(&self) -> ProtocolId {
        self.protocol_id.clone()
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let requester_is_owner = peer == self.deps.keypair.peer_id();
        let first = read_frame(&mut stream).await?;
        if first.is_empty() {
            return Ok(()); // 对端关流
        }
        // 首帧嗅探（§5：card 相与 task 相各自独立开流）：card 帧带 op，
        // task 帧是 JSON-RPC（jsonrpc 字段）。
        let is_task = serde_json::from_slice::<serde_json::Value>(&first)
            .ok()
            .is_some_and(|v| v.get("op").is_none());
        if is_task {
            return super::stream::serve_task_stream(
                self.deps.tasks.clone(),
                stream,
                peer,
                requester_is_owner,
                first,
            )
            .await;
        }
        let Ok(frame) = serde_json::from_slice::<CardFrame>(&first) else {
            self.deps.audit.record(crate::audit::AuditEvent::A2aDenied {
                peer: peer.to_string(),
                detail: "first card frame unparsable".into(),
            });
            return Ok(());
        };
        self.card_loop(peer, requester_is_owner, frame, stream)
            .await
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        // 裸流无身份上下文（acp-over-p2p §4.1 身份补记同款 fail-closed）
        Err(io::Error::other("a2a stream requires peer identity"))
    }
}

impl A2aHandler {
    /// card 相循环：请求-响应 + 订阅态推送双向合流。
    async fn card_loop(
        &self,
        peer: PeerId,
        requester_is_owner: bool,
        first: CardFrame,
        mut stream: BoxedStream,
    ) -> io::Result<()> {
        let mut rx: Option<mpsc::Receiver<CardFrame>> = None;
        let mut pending = Some(first);
        loop {
            // 订阅态下同时消费推送通道；未订阅只读请求
            let push = async {
                match rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                push = push => {
                    if let Some(frame) = push {
                        let bytes = serde_json::to_vec(&frame).map_err(io::Error::other)?;
                        write_frame(&mut stream, &bytes).await?;
                    }
                }
                req = async {
                    match pending.take() {
                        Some(frame) => Ok(Some(frame)),
                        None => read_frame(&mut stream).await.map(|bytes| {
                            if bytes.is_empty() {
                                None
                            } else {
                                serde_json::from_slice::<CardFrame>(&bytes).ok()
                            }
                        }),
                    }
                } => {
                    let Some(frame) = req? else { return Ok(()) };
                    for reply in self.dispatch(peer, requester_is_owner, frame, &mut rx) {
                        let bytes = serde_json::to_vec(&reply).map_err(io::Error::other)?;
                        write_frame(&mut stream, &bytes).await?;
                    }
                }
            }
        }
    }

    fn dispatch(
        &self,
        peer: PeerId,
        requester_is_owner: bool,
        frame: CardFrame,
        rx: &mut Option<mpsc::Receiver<CardFrame>>,
    ) -> Vec<CardFrame> {
        match frame {
            CardFrame::List { id, .. } => {
                let cards = self.visible_cards(&peer, requester_is_owner);
                vec![CardFrame::Cards {
                    v: a2a::CARD_FRAME_VERSION,
                    id,
                    cards,
                }]
            }
            CardFrame::Get { id, agent_id, .. } => {
                let cards = self.visible_cards(&peer, requester_is_owner);
                let found: Vec<SignedCard> = cards
                    .into_iter()
                    .filter(|c| c.0.payload.agent_id == agent_id)
                    .collect();
                if found.is_empty() {
                    vec![CardFrame::Error {
                        v: a2a::CARD_FRAME_VERSION,
                        id,
                        code: "not-found".into(),
                        message: format!("agent {agent_id} 不可见或不存在"),
                    }]
                } else {
                    vec![CardFrame::Cards {
                        v: a2a::CARD_FRAME_VERSION,
                        id,
                        cards: found,
                    }]
                }
            }
            CardFrame::Subscribe { id, .. } => {
                *rx = Some(self.deps.subscribers.register(&peer.to_string()));
                // 应答 + 当前快照（设计 §5.1：subscribe 应答含快照）
                let cards = self.visible_cards(&peer, requester_is_owner);
                vec![
                    CardFrame::Ok {
                        v: a2a::CARD_FRAME_VERSION,
                        id,
                        subscribed: true,
                    },
                    CardFrame::Push {
                        v: a2a::CARD_FRAME_VERSION,
                        id: 0,
                        cards,
                        removed: vec![],
                    },
                ]
            }
            // 邀请帧处理（design §7.3）
            CardFrame::InviteRequest { v, id, invite } => {
                self.handle_invite_request(peer, requester_is_owner, v, id, invite)
            }
            CardFrame::InviteReceipt { v, id, receipt } => {
                self.handle_invite_receipt(peer, requester_is_owner, v, id, receipt)
            }
            // 客户端不应发送服务端帧；收到即协议违规（不回应，读端随后 EOF 断流）
            CardFrame::Cards { .. }
            | CardFrame::Ok { .. }
            | CardFrame::Push { .. }
            | CardFrame::InviteResponse { .. }
            | CardFrame::InviteReceiptResponse { .. } => {
                self.deps.audit.record(crate::audit::AuditEvent::A2aDenied {
                    peer: peer.to_string(),
                    detail: "client sent server-only card frame".into(),
                });
                vec![]
            }
            CardFrame::Error { .. } => vec![],
        }
    }

    /// 处理邀请请求：owner 生成签名凭证邀请帧。
    fn handle_invite_request(
        &self,
        peer: PeerId,
        requester_is_owner: bool,
        v: u8,
        id: u64,
        invite: serde_json::Value,
    ) -> Vec<CardFrame> {
        // 只有 owner 可以发起邀请
        if !requester_is_owner {
            self.deps.audit.record(crate::audit::AuditEvent::A2aDenied {
                peer: peer.to_string(),
                detail: "non-owner invite request rejected".into(),
            });
            return vec![CardFrame::InviteResponse {
                v,
                id,
                invite: None,
                error: Some("denied".into()),
                message: Some("only owner can create invites".into()),
            }];
        }
        // 解析邀请帧
        let invite_frame: a2a::InviteFrame = match serde_json::from_value(invite) {
            Ok(f) => f,
            Err(e) => {
                return vec![CardFrame::InviteResponse {
                    v,
                    id,
                    invite: None,
                    error: Some("bad-invite".into()),
                    message: Some(format!("invite parse error: {e}")),
                }];
            }
        };
        // 校验邀请帧签名和时间窗
        let now = unix_now();
        if let Err(e) = a2a::verify_invite(&invite_frame, &invite_frame.payload.invitee_peer, now) {
            return vec![CardFrame::InviteResponse {
                v,
                id,
                invite: None,
                error: Some("bad-invite".into()),
                message: Some(format!("invite verify error: {e}")),
            }];
        }
        // 检查 nonce 一次性
        if self.deps.invites.is_nonce_used(&invite_frame.payload.nonce) {
            return vec![CardFrame::InviteResponse {
                v,
                id,
                invite: None,
                error: Some("nonce-used".into()),
                message: Some("nonce already used".into()),
            }];
        }
        // 持久化邀请
        let entry = crate::a2a::invites::InviteEntry {
            nonce: invite_frame.payload.nonce.clone(),
            agent_id: invite_frame.payload.card.0.payload.agent_id.clone(),
            host_peer: self.deps.host_peer.clone(),
            invitee_peer: invite_frame.payload.invitee_peer.clone(),
            expiry: invite_frame.payload.expiry,
            issued_at: invite_frame.issued_at,
            status: crate::a2a::invites::InviteStatus::Pending,
            receipt_sig: None,
        };
        if let Err(e) = self.deps.invites.insert(entry) {
            return vec![CardFrame::InviteResponse {
                v,
                id,
                invite: None,
                error: Some("store-error".into()),
                message: Some(format!("invite store error: {e}")),
            }];
        }
        // 返回邀请帧
        let invite_json = serde_json::to_value(&invite_frame).unwrap_or_default();
        vec![CardFrame::InviteResponse {
            v,
            id,
            invite: Some(invite_json),
            error: None,
            message: None,
        }]
    }

    /// 处理邀请回执：invitee 提交签名回执。
    fn handle_invite_receipt(
        &self,
        _peer: PeerId,
        _requester_is_owner: bool,
        v: u8,
        id: u64,
        receipt: serde_json::Value,
    ) -> Vec<CardFrame> {
        // 解析回执
        let receipt_signed: p2p_identity::signed::Signed<a2a::ReceiptPayload> =
            match serde_json::from_value(receipt) {
                Ok(r) => r,
                Err(e) => {
                    return vec![CardFrame::InviteReceiptResponse {
                        v,
                        id,
                        ok: false,
                        message: Some(format!("receipt parse error: {e}")),
                    }];
                }
            };
        // 查找对应邀请
        let invites = self.deps.invites.list();
        let invite = invites
            .iter()
            .find(|i| i.nonce == receipt_signed.payload.nonce);
        let Some(invite) = invite else {
            return vec![CardFrame::InviteReceiptResponse {
                v,
                id,
                ok: false,
                message: Some("nonce not found".into()),
            }];
        };
        // 验证回执签名
        let now = unix_now();
        if let Err(e) = a2a::verify_receipt(
            &receipt_signed,
            &invite.nonce,
            &invite.agent_id,
            &invite.host_peer,
            now,
        ) {
            return vec![CardFrame::InviteReceiptResponse {
                v,
                id,
                ok: false,
                message: Some(format!("receipt verify error: {e}")),
            }];
        }
        // 验证 invitee 绑定（回执签名者必须是 invitee）
        let receipt_peer = PeerId::from_public_key(&receipt_signed.pubkey);
        if receipt_peer.to_string() != invite.invitee_peer {
            return vec![CardFrame::InviteReceiptResponse {
                v,
                id,
                ok: false,
                message: Some("receipt signer mismatch invitee".into()),
            }];
        }
        // 标记接受
        let receipt_sig = format!("{:?}", receipt_signed.sig);
        if let Err(e) = self.deps.invites.accept(&invite.nonce, &receipt_sig) {
            return vec![CardFrame::InviteReceiptResponse {
                v,
                id,
                ok: false,
                message: Some(format!("accept error: {e}")),
            }];
        }
        // 写入授权清单
        if let Err(e) = self
            .deps
            .tasks
            .grants
            .grant(&invite.agent_id, &invite.invitee_peer, now)
        {
            return vec![CardFrame::InviteReceiptResponse {
                v,
                id,
                ok: false,
                message: Some(format!("grant error: {e}")),
            }];
        }
        vec![CardFrame::InviteReceiptResponse {
            v,
            id,
            ok: true,
            message: None,
        }]
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
