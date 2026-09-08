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
        self.deps
            .agents
            .signed_cards_for(&self.deps.keypair, &self.deps.host_peer, requester_is_owner, &granted, now)
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
        self.card_loop(peer, requester_is_owner, frame, stream).await
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
            // 客户端不应发送服务端帧；收到即协议违规（不回应，读端随后 EOF 断流）
            CardFrame::Cards { .. } | CardFrame::Ok { .. } | CardFrame::Push { .. } => {
                self.deps.audit.record(crate::audit::AuditEvent::A2aDenied {
                    peer: peer.to_string(),
                    detail: "client sent server-only card frame".into(),
                });
                vec![]
            }
            CardFrame::Error { .. } => vec![],
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
