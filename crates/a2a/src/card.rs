//! AgentCard 模型（docs/design/a2a-over-p2p-design.md §4）：A2A 对齐字段 + p2p 扩展。
//! 能力真相原则：capabilities/skills 是宿主自述，GUI「未声明=不支持」灰化渲染，本库不代答。

use p2p_identity::{
    signed::{self, Signed},
    Keypair, PeerId,
};
use serde::{Deserialize, Serialize};

/// agentId 字符集与长度：仅小写字母/数字/连字符，≤32（防非 ASCII 与超长卡键）。
pub const AGENT_ID_MAX_CHARS: usize = 32;
/// 卡片 TTL 上限（秒）：服务端 rendezvous 封顶 3600，订阅侧钳制同值。
pub const TTL_MAX_SECS: u64 = 3600;
/// 卡片 TTL 默认（秒）：发现刷新周期基准（拍板 Q4）。
pub const TTL_DEFAULT_SECS: u64 = 300;
/// skills 条数上限（GUI chip 输入同口径）。
pub const SKILLS_MAX: usize = 10;

/// 可见性：public=全网可发现可聊；private=仅授权 peer；local=仅 owner（loopback）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Local,
}

/// 能力位自述（v1 只声明 streaming；其余按需加法）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCapabilities {
    #[serde(default)]
    pub streaming: bool,
}

/// 技能项：id 必填（[a-z0-9-]），name 必填，description/tags 可选。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// AgentCard：宿主签名前的声明本体（wire 字段 camelCase，对齐设计稿 JSON）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AgentCard {
    pub agent_id: String,
    pub name: String,
    pub description: String,
    /// a2a://<hostPeer>/<agentId> 逻辑地址。
    pub url: String,
    /// 宿主 PeerId（base58），= 签名 pubkey 推导值（§4.2 绑定校验）。
    pub host_peer: String,
    pub visibility: Visibility,
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,
    /// 声明有效期（秒）：0 < ttl ≤ 3600。
    pub ttl_secs: u64,
    /// 卡片版本：AgentBook 按版本升序替换（防旧卡覆盖新卡）。
    pub version: u64,
}

/// 卡片校验错误：全部显式，失败路径不留静默。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CardError {
    #[error("{0} must not be empty")]
    Empty(&'static str),
    #[error("agent_id invalid: {0}（仅 [a-z0-9-]，≤{AGENT_ID_MAX_CHARS} 字符）")]
    AgentIdInvalid(String),
    #[error("host_peer 不是合法 base58 PeerId")]
    HostPeerInvalid,
    #[error("url 必须以 a2a:// 开头且含 hostPeer/agentId")]
    UrlInvalid,
    #[error("ttl_secs 必须在 (0, {TTL_MAX_SECS}] 区间")]
    TtlRange,
    #[error("skills 超过上限 {SKILLS_MAX} 条")]
    TooManySkills,
    #[error("skill id 非法（仅 [a-z0-9-]）: {0}")]
    SkillIdInvalid(String),
    #[error("host_peer 与签名 pubkey 绑定失败")]
    PeerMismatch,
    #[error("信封签名/时间窗校验失败: {0}")]
    Envelope(String),
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= AGENT_ID_MAX_CHARS
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl AgentCard {
    /// 声明合法性：非空字段、agentId/skill id 字符集、url 形状、TTL 区间、skills 条数。
    pub fn validate(&self) -> Result<(), CardError> {
        if self.agent_id.is_empty() {
            return Err(CardError::Empty("agent_id"));
        }
        if !is_slug(&self.agent_id) {
            return Err(CardError::AgentIdInvalid(self.agent_id.clone()));
        }
        if self.name.is_empty() {
            return Err(CardError::Empty("name"));
        }
        if self.description.is_empty() {
            return Err(CardError::Empty("description"));
        }
        if self.skills.len() > SKILLS_MAX {
            return Err(CardError::TooManySkills);
        }
        for skill in &self.skills {
            if !is_slug(&skill.id) {
                return Err(CardError::SkillIdInvalid(skill.id.clone()));
            }
            if skill.name.is_empty() {
                return Err(CardError::Empty("skill.name"));
            }
        }
        let prefix = format!("a2a://{}/", self.host_peer);
        if !self.url.starts_with(&prefix) {
            return Err(CardError::UrlInvalid);
        }
        if self.ttl_secs == 0 || self.ttl_secs > TTL_MAX_SECS {
            return Err(CardError::TtlRange);
        }
        Ok(())
    }

    /// 宿主 PeerId 解析：host_peer base58 解码失败即声明损坏。
    pub fn host_peer_id(&self) -> Option<PeerId> {
        let raw: [u8; 32] = bs58::decode(&self.host_peer)
            .into_vec()
            .ok()?
            .try_into()
            .ok()?;
        Some(PeerId::from_bytes(raw))
    }
}

/// 签名卡片信封：p2p-identity::signed::Signed<AgentCard> + hostPeer 绑定。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedCard(pub Signed<AgentCard>);

impl SignedCard {
    /// 签发：先校验声明合法性，拒签非法卡片。
    pub fn sign(card: AgentCard, kp: &Keypair, now: u64) -> Result<Self, CardError> {
        card.validate()?;
        let signed = Signed::sign(card, kp, now).map_err(|e| CardError::Envelope(e.to_string()))?;
        Ok(Self(signed))
    }

    /// 宿主 PeerId（卡片声明）。
    pub fn host_peer_id(&self) -> Option<PeerId> {
        self.0.payload.host_peer_id()
    }

    /// 卡片键：hostPeer + agentId 复合（AgentBook 键空间）。
    pub fn key(&self) -> Option<AgentKey> {
        Some(AgentKey {
            host: self.host_peer_id()?,
            agent_id: self.0.payload.agent_id.clone(),
        })
    }

    /// 过期时刻（unix 秒）。
    pub fn expires_at(&self) -> u64 {
        self.0.issued_at.saturating_add(self.0.payload.ttl_secs)
    }

    /// 离线验证：pubkey 绑定 hostPeer + 签名 + 时间窗（now ∈ [issued_at, expires)）。
    pub fn verify(&self, now: u64) -> Result<(), CardError> {
        let host = self.host_peer_id().ok_or(CardError::HostPeerInvalid)?;
        signed::check_identity(&self.0.pubkey, &host).map_err(|_| CardError::PeerMismatch)?;
        self.0
            .verify(self.0.payload.ttl_secs, now)
            .map_err(|e| CardError::Envelope(e.to_string()))
    }
}

/// AgentBook 键：同 host 可多 agent，复合键区分。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentKey {
    pub host: PeerId,
    pub agent_id: String,
}

impl std::fmt::Display for AgentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.host, self.agent_id)
    }
}
