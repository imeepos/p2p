//! 线上信封（wire.rs 行数红线拆分）：编解码形状与出入站转换。
//!
//! 对端身份说明：底座 handler 拿不到对端 PeerId（serve.rs 分发不携带 peer），
//! 故线上 peer 字段承载发端自身 PeerId；收端校验其合法、非本机且 sender 为 me
//! （sender=them 即冒充本机）——内核只读约束下的纵深防御上限，流安全由底座保证。

use p2p_identity::PeerId;

use crate::model::{
    parse_peer_id, validate_media, validate_text, ChatEnvelope, ChatError, ChatKind, ChatMediaMeta,
    ChatStatus, Sender,
};

/// 线上信封：peer = 发端自身 PeerId；status/path 为本地字段不跨网。
/// fromAddrs 加法字段（F1 地址自学习）：发端声明地址，收端回写好友簿；
/// serde(default) 保证旧对端缺字段可读，旧对端收新字段也忽略（双向兼容）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireEnvelope {
    id: String,
    peer: String,
    sender: Sender,
    kind: ChatKind,
    #[serde(rename = "tsMs")]
    ts_ms: i64,
    text: Option<String>,
    media: Option<WireMedia>,
    reply_to: Option<String>,
    /// 入群邀请卡片载荷（IMC1 加法字段，kind=groupinvite 时有值；旧对端忽略）。
    #[serde(default)]
    pub(crate) card: Option<crate::ginvite::ChatInviteCard>,
    #[serde(default)]
    pub(crate) from_addrs: Option<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireMedia {
    name: String,
    mime: String,
    size: u64,
}

impl WireEnvelope {
    /// 发端声明地址取 serve 发布的 advertised（空则不携带）：一次性进程的
    /// 即弃监听地址禁止上 wire，防对端好友簿被污染。
    pub(crate) fn from_outbound(
        env: &ChatEnvelope,
        local: PeerId,
        from_addrs: Vec<String>,
    ) -> Self {
        Self {
            id: env.id.clone(),
            peer: local.to_string(),
            sender: Sender::Me,
            kind: env.kind.clone(),
            ts_ms: env.ts_ms,
            text: env.text.clone(),
            media: env.media.as_ref().map(|m| WireMedia {
                name: m.name.clone(),
                mime: m.mime.clone(),
                size: m.size,
            }),
            reply_to: env.reply_to.clone(),
            card: env.card.clone(),
            from_addrs: (!from_addrs.is_empty()).then_some(from_addrs),
        }
    }

    /// 入站校验并转存储信封：sender 必须为 me、peer 合法且非本机。
    pub(crate) fn into_inbound(self, local: PeerId) -> Result<ChatEnvelope, ChatError> {
        if self.sender != Sender::Me {
            return Err(ChatError::Protocol(
                "入站信封 sender 非 me（对端视角），疑似伪装".into(),
            ));
        }
        let peer_id = parse_peer_id(&self.peer)?;
        if peer_id == local {
            return Err(ChatError::Protocol(
                "入站信封 peer 指向本机，疑似伪装".into(),
            ));
        }
        match self.kind {
            ChatKind::Text => {
                let text = validate_text(self.text.as_deref().unwrap_or_default())?;
                if self.media.is_some() {
                    return Err(ChatError::InvalidMedia("text 消息携带附件，拒绝".into()));
                }
                Ok(ChatEnvelope {
                    id: self.id,
                    peer: self.peer,
                    sender: Sender::Them,
                    kind: ChatKind::Text,
                    ts_ms: self.ts_ms,
                    text: Some(text),
                    media: None,
                    status: ChatStatus::Delivered,
                    reply_to: self.reply_to,
                    card: None,
                })
            }
            ChatKind::GroupInvite => {
                if self.media.is_some() {
                    return Err(ChatError::InvalidMedia(
                        "groupinvite 卡片不接受附件，拒绝".into(),
                    ));
                }
                let card = self.card.ok_or_else(|| {
                    ChatError::InvalidMedia("groupinvite 卡片缺载荷，拒绝".into())
                })?;
                Ok(ChatEnvelope {
                    id: self.id,
                    peer: self.peer,
                    sender: Sender::Them,
                    kind: ChatKind::GroupInvite,
                    ts_ms: self.ts_ms,
                    text: None,
                    media: None,
                    status: ChatStatus::Delivered,
                    reply_to: self.reply_to,
                    card: Some(card),
                })
            }
            kind => {
                let m = self
                    .media
                    .ok_or_else(|| ChatError::InvalidMedia(format!("{kind} 消息缺附件，拒绝")))?;
                validate_media(&kind, &m.mime, m.size)?;
                Ok(ChatEnvelope {
                    id: self.id,
                    peer: self.peer,
                    sender: Sender::Them,
                    kind,
                    ts_ms: self.ts_ms,
                    text: None,
                    media: Some(ChatMediaMeta {
                        name: m.name,
                        mime: m.mime,
                        size: m.size,
                        path: None,
                    }),
                    status: ChatStatus::Delivered,
                    reply_to: self.reply_to,
                    card: None,
                })
            }
        }
    }
}
