//! 订阅侧卡片簿（docs/design/a2a-over-p2p-design.md §4.3）：验签 + 钳制 + 版本替换。
//! 入簿三重防线：签名/身份/时间窗（SignedCard::verify）+ issued_at 偏差钳制 + TTL 上限钳制；
//! 容量上限按最旧驱逐；版本升序替换防旧卡覆盖。

use std::collections::{HashMap, HashSet, VecDeque};

use p2p_identity::PeerId;
use tracing::warn;

use crate::card::{AgentKey, SignedCard, TTL_MAX_SECS};

/// 订阅侧容量上限：公开池规模 512 认知（design §12），本簿收敛为 256。
pub const BOOK_MAX_ENTRIES: usize = 256;
/// issued_at 偏差钳制（秒）：防未来/过旧时间戳击穿时间窗。
pub const ISSUED_AT_MAX_SKEW_SECS: u64 = 300;

/// 入簿错误：验签/钳制/版本三类，全部显式。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BookError {
    #[error("card 验签失败: {0}")]
    Verify(String),
    #[error("issued_at 偏差超限（>{ISSUED_AT_MAX_SKEW_SECS}s）")]
    Skew,
    #[error("ttl_secs 超上限（>{TTL_MAX_SECS}s）")]
    TtlTooLong,
    #[error("卡片键损坏（hostPeer 非法）")]
    BadKey,
    #[error("旧版本卡片（版本不升序）")]
    StaleVersion,
}

#[derive(Debug, Default)]
pub struct AgentBook {
    entries: HashMap<AgentKey, SignedCard>,
    /// 容量驱逐按插入序（最旧优先）。
    order: VecDeque<AgentKey>,
}

impl AgentBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// 验签 + 钳制 + 版本替换后入簿；任一不过都拒，失败路径留显式错误。
    pub fn insert(&mut self, card: SignedCard, now: u64) -> Result<(), BookError> {
        let skew = card.0.issued_at.abs_diff(now);
        if skew > ISSUED_AT_MAX_SKEW_SECS {
            return Err(BookError::Skew);
        }
        if card.0.payload.ttl_secs > TTL_MAX_SECS {
            return Err(BookError::TtlTooLong);
        }
        card.verify(now)
            .map_err(|e| BookError::Verify(e.to_string()))?;
        let key = card.key().ok_or(BookError::BadKey)?;
        if let Some(existing) = self.entries.get(&key) {
            if existing.0.payload.version >= card.0.payload.version {
                return Err(BookError::StaleVersion);
            }
            // 版本升序替换：保持原插入位置（不重复入队）
            self.entries.insert(key, card);
            return Ok(());
        }
        if self.entries.len() >= BOOK_MAX_ENTRIES {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
                warn!(target: "a2a_book", key = %oldest, "capacity evicted");
            }
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, card);
        Ok(())
    }

    /// 当前仍有效的卡片（按卡片 TTL 过滤）。
    pub fn live(&self, now: u64) -> Vec<&SignedCard> {
        self.entries
            .values()
            .filter(|c| now < c.expires_at())
            .collect()
    }

    /// 移除过期卡片并返回键清单（对应 Expired 语义），WARN 留观测信号。
    pub fn evict_expired(&mut self, now: u64) -> Vec<AgentKey> {
        let expired: Vec<AgentKey> = self
            .entries
            .iter()
            .filter(|(_, c)| now >= c.expires_at())
            .map(|(k, _)| k.clone())
            .collect();
        for key in &expired {
            self.entries.remove(key);
            warn!(target: "a2a_book", key = %key, "card expired, evicted");
        }
        self.order.retain(|k| self.entries.contains_key(k));
        expired
    }

    /// 按请求方可见性过滤（design §5.1/§9 F4）：public 全集；private/local 仅授权。
    /// is_granted 由宿主按 viewer 的授权清单判定（簿不持有授权语义）。
    pub fn visible_to(
        &self,
        now: u64,
        is_granted: &impl Fn(&PeerId, &str) -> bool,
    ) -> Vec<&SignedCard> {
        self.entries
            .values()
            .filter(|c| {
                if now >= c.expires_at() {
                    return false;
                }
                match c.0.payload.visibility {
                    crate::card::Visibility::Public => true,
                    crate::card::Visibility::Private | crate::card::Visibility::Local => c
                        .host_peer_id()
                        .is_some_and(|host| is_granted(&host, &c.0.payload.agent_id)),
                }
            })
            .collect()
    }

    pub fn get(&self, key: &AgentKey) -> Option<&SignedCard> {
        self.entries.get(key)
    }

    /// 移除单卡（card/remove 帧处理）。
    pub fn remove(&mut self, key: &AgentKey) -> bool {
        let removed = self.entries.remove(key).is_some();
        if removed {
            self.order.retain(|k| k != key);
        }
        removed
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 全部键（push/remove 同步用）。
    pub fn keys(&self) -> HashSet<AgentKey> {
        self.entries.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{AgentCapabilities, AgentCard, AgentSkill, SignedCard, Visibility};
    use p2p_identity::Keypair;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn card_for(kp: &Keypair, agent_id: &str, visibility: Visibility, version: u64) -> SignedCard {
        let host = kp.peer_id().to_string();
        let card = AgentCard {
            agent_id: agent_id.into(),
            name: "agent".into(),
            description: "desc".into(),
            url: format!("a2a://{host}/{agent_id}"),
            host_peer: host,
            visibility,
            capabilities: AgentCapabilities::default(),
            skills: vec![AgentSkill {
                id: "s1".into(),
                name: "s1".into(),
                description: None,
                tags: vec![],
            }],
            ttl_secs: crate::card::TTL_DEFAULT_SECS,
            version,
        };
        SignedCard::sign(card, kp, now()).expect("sign")
    }

    #[test]
    fn insert_verify_clamp_and_version() {
        let kp = Keypair::generate();
        let t = now();
        let mut book = AgentBook::new();
        book.insert(card_for(&kp, "a1", Visibility::Public, 1), t)
            .unwrap();
        assert_eq!(book.len(), 1);
        // 旧版本拒
        assert_eq!(
            book.insert(card_for(&kp, "a1", Visibility::Public, 1), t),
            Err(BookError::StaleVersion)
        );
        // 新版本替换
        book.insert(card_for(&kp, "a1", Visibility::Public, 2), t)
            .unwrap();
        assert_eq!(
            book.get(&card_for(&kp, "a1", Visibility::Public, 2).key().unwrap())
                .map(|c| c.0.payload.version),
            Some(2)
        );
    }

    #[test]
    fn skew_and_ttl_clamped() {
        let kp = Keypair::generate();
        let t = now();
        let mut book = AgentBook::new();
        // 未来 10 分钟签发 → skew 拒
        let future = card_for(&kp, "f1", Visibility::Public, 1);
        let mut future = future;
        future.0.issued_at = t + 600;
        assert_eq!(book.insert(future, t), Err(BookError::Skew));
        // TTL 超上限拒
        let mut long = card_for(&kp, "f2", Visibility::Public, 1);
        long.0.payload.ttl_secs = TTL_MAX_SECS + 1;
        assert_eq!(book.insert(long, t), Err(BookError::TtlTooLong));
    }

    #[test]
    fn visibility_filter() {
        let kp = Keypair::generate();
        let t = now();
        let mut book = AgentBook::new();
        book.insert(card_for(&kp, "pub", Visibility::Public, 1), t)
            .unwrap();
        book.insert(card_for(&kp, "priv", Visibility::Private, 1), t)
            .unwrap();
        book.insert(card_for(&kp, "loc", Visibility::Local, 1), t)
            .unwrap();
        // 授权函数全部拒绝：仅 public 可见，private/local 必须授权才出现
        let vis = book.visible_to(t, &|_: &PeerId, _: &str| false);
        assert_eq!(vis.len(), 1, "仅 public 可见");
        assert_eq!(vis[0].0.payload.agent_id, "pub");
        // 授权匹配 host 与 agentId 后，private 出现、local 仍拒绝（local 仅 owner，由宿主判定）
        let vis_granted = book.visible_to(t, &|host: &PeerId, id: &str| {
            host == &kp.peer_id() && id == "priv"
        });
        assert_eq!(vis_granted.len(), 2, "public + 已授权 private");
    }

    #[test]
    fn expiry_and_remove() {
        let kp = Keypair::generate();
        let t = now();
        let mut book = AgentBook::new();
        let card = card_for(&kp, "e1", Visibility::Public, 1);
        let key = card.key().unwrap();
        book.insert(card, t).unwrap();
        // 过期：TTL 之后 live 为空、evict 出键
        assert!(book.live(t + crate::card::TTL_DEFAULT_SECS).is_empty());
        let evicted = book.evict_expired(t + crate::card::TTL_DEFAULT_SECS);
        assert!(evicted.contains(&key));
        assert!(book.is_empty());
        // 移除
        book.insert(card_for(&kp, "e2", Visibility::Public, 1), t)
            .unwrap();
        let key2 = book.keys().into_iter().next().unwrap();
        assert!(book.remove(&key2));
        assert!(!book.remove(&key2));
    }
}
