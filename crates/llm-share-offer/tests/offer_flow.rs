//! 机械验收单测（E10-T18）：声明校验拒绝非法值 / TTL 过期失效 / 选路过滤与排序 /
//! 签名注册往返（走 p2p-discovery 公开设施 RendezvousRegistry，进程内即往返）。

use std::collections::BTreeMap;
use std::net::IpAddr;

use llm_share_offer::{
    announce_register, select_offers, Offer, OfferBook, OfferError, RateLimit, SignedOffer,
    VerifyError, OFFER_NAMESPACE,
};
use p2p_discovery::rendezvous::messages::Query;
use p2p_discovery::rendezvous::RendezvousRegistry;
use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;

const T0: u64 = 1_700_000_000;

fn offer(kp: &Keypair, model: &str, spare: u64) -> Offer {
    let mut spare_map = BTreeMap::new();
    spare_map.insert(model.to_string(), spare);
    Offer {
        peer: kp.peer_id().to_string(),
        models: vec![model.to_string()],
        spare: spare_map,
        period_ends: "2026-09-30".to_string(),
        max_per_req: BTreeMap::new(),
        rate_limit: RateLimit {
            rpm: 10,
            concurrency: 2,
        },
        ttl_secs: 60,
        retention: "none".to_string(),
    }
}

fn sign_valid(kp: &Keypair, model: &str, spare: u64, issued_at: u64) -> SignedOffer {
    SignedOffer::sign(&offer(kp, model, spare), kp, issued_at).unwrap()
}

fn quic_localhost(port: u16) -> TransportAddr {
    TransportAddr::Quic {
        ip: "127.0.0.1".parse::<IpAddr>().unwrap(),
        port,
    }
}

#[test]
fn validation_rejects_illegal_values() {
    let kp = Keypair::generate();
    let base = offer(&kp, "gpt-4o", 100);
    assert_eq!(base.validate(), Ok(()));

    let mut o = base.clone();
    o.peer = String::new();
    assert_eq!(o.validate(), Err(OfferError::Empty("peer")));

    let mut o = base.clone();
    o.retention = String::new();
    assert_eq!(o.validate(), Err(OfferError::Empty("retention")));

    let mut o = base.clone();
    o.models.clear();
    assert_eq!(o.validate(), Err(OfferError::Empty("models")));

    let mut o = base.clone();
    o.spare.insert("gpt-4o".to_string(), 0);
    assert_eq!(
        o.validate(),
        Err(OfferError::SparePositive("gpt-4o".into()))
    );

    let mut o = base.clone();
    o.spare.remove("gpt-4o");
    assert_eq!(
        o.validate(),
        Err(OfferError::SparePositive("gpt-4o".into()))
    );

    let mut o = base.clone();
    o.max_per_req.insert("unknown".to_string(), 5);
    assert_eq!(
        o.validate(),
        Err(OfferError::UnknownModel("unknown".into()))
    );

    let mut o = base.clone();
    o.rate_limit.rpm = 0;
    assert_eq!(o.validate(), Err(OfferError::Positive("rate_limit")));

    let mut o = base.clone();
    o.ttl_secs = 0;
    assert_eq!(o.validate(), Err(OfferError::Positive("ttl")));

    // 非法声明拒签
    assert!(SignedOffer::sign(&o, &kp, T0).is_err());
}

#[test]
fn ttl_expiry_invalidates_offer() {
    let kp = Keypair::generate();
    let signed = sign_valid(&kp, "gpt-4o", 100, T0); // ttl 60

    // 时间窗边界：T0-1 未生效，[T0, T0+60) 有效，T0+60 过期
    assert_eq!(signed.verify(T0 - 1), Err(VerifyError::NotYetValid));
    assert_eq!(signed.verify(T0), Ok(()));
    assert_eq!(signed.verify(T0 + 59), Ok(()));
    assert_eq!(signed.verify(T0 + 60), Err(VerifyError::Expired(T0 + 60)));
    assert_eq!(signed.expires_at(), T0 + 60);

    // 簿内：过期后 live 不再返回，evict 清出对应 peer
    let mut book = OfferBook::new();
    assert_eq!(book.insert(signed, T0), Ok(()));
    assert_eq!(book.live(T0 + 59).len(), 1);
    assert!(book.live(T0 + 60).is_empty());
    assert_eq!(book.evict_expired(T0 + 60), vec![kp.peer_id()]);
    assert!(book.is_empty());
}

#[test]
fn envelope_rejects_tamper_and_foreign_peer() {
    let kp = Keypair::generate();
    let other = Keypair::generate();

    let mut tampered = sign_valid(&kp, "gpt-4o", 100, T0);
    tampered.offer.retention = "metadata-90d".to_string();
    assert_eq!(tampered.verify(T0), Err(VerifyError::BadSignature));

    let mut bad_sig = sign_valid(&kp, "gpt-4o", 100, T0);
    bad_sig.sig[0] ^= 0xff;
    assert_eq!(bad_sig.verify(T0), Err(VerifyError::BadSignature));

    // 声明 peer 与签名公钥不一致（冒名）
    let forged = SignedOffer::sign(&offer(&kp, "gpt-4o", 100), &other, T0).unwrap();
    assert_eq!(forged.verify(T0), Err(VerifyError::PeerMismatch));

    // JSON 往返（b58 编排 pubkey/sig）后语义不变
    let signed = sign_valid(&kp, "gpt-4o", 100, T0);
    let json = serde_json::to_string(&signed).unwrap();
    let back: SignedOffer = serde_json::from_str(&json).unwrap();
    assert_eq!(back, signed);
    assert_eq!(back.verify(T0), Ok(()));
}

#[test]
fn routing_filters_and_sorts() {
    let ka = Keypair::generate();
    let kb = Keypair::generate();
    let kc = Keypair::generate();
    let kd = Keypair::generate();

    let mut a = offer(&ka, "gpt-4o", 100);
    a.retention = "none".to_string();
    let mut b = offer(&kb, "gpt-4o", 500);
    b.retention = "metadata-90d".to_string();
    b.max_per_req.insert("gpt-4o".to_string(), 32_000);

    let sa = SignedOffer::sign(&a, &ka, T0).unwrap();
    let sb = SignedOffer::sign(&b, &kb, T0).unwrap();
    // 模型不匹配
    let sc = sign_valid(&kc, "deepseek-v3", 999, T0);
    // issued_at 早于 T0 两个 TTL，选路时刻已过期
    let sd = sign_valid(&kd, "gpt-4o", 50, T0 - 120);

    let offers = vec![sa, sb, sc, sd];
    let picked = select_offers(&offers, "gpt-4o", T0 + 30);
    assert_eq!(picked.len(), 2);
    assert_eq!(picked[0].peer, kb.peer_id().to_string());
    assert_eq!(picked[0].spare, 500);
    assert_eq!(picked[0].max_per_req, Some(32_000));
    assert_eq!(picked[0].retention, "metadata-90d");
    assert_eq!(picked[1].peer, ka.peer_id().to_string());
    assert_eq!(picked[1].max_per_req, None);
    assert_eq!(picked[1].retention, "none");

    assert!(select_offers(&offers, "claude-3", T0).is_empty());
    // now 越过全部 TTL：无候选（A5）
    assert!(select_offers(&offers, "gpt-4o", T0 + 61).is_empty());
}

#[test]
fn signed_register_roundtrip_via_registry() {
    let kp = Keypair::generate();
    let addrs = vec![quic_localhost(4001)];
    let reg = announce_register(&kp, &addrs, 60, T0);
    assert_eq!(reg.namespace, OFFER_NAMESPACE);

    // 签名注册 -> 服务端校验入库 -> namespace 查询往返
    let registry = RendezvousRegistry::new();
    assert_eq!(registry.register(&reg, T0), Ok(()));
    let mut resp = registry.query(&Query {
        namespace: OFFER_NAMESPACE.to_string(),
        peer_id: Vec::new(),
    });
    assert_eq!(resp.error, "");
    assert_eq!(resp.peers.len(), 1);
    let entry = resp.peers.remove(0);
    let raw: [u8; 32] = entry.peer_id.try_into().unwrap();
    assert_eq!(PeerId::from_bytes(raw), kp.peer_id());
    assert_eq!(entry.addrs.len(), 1);

    // 超出新鲜度窗口（±300s）被拒
    assert!(registry.register(&reg, T0 + 400).is_err());

    // 篡改 TTL 后签名不再成立：注册被拒（签名覆盖 namespace/peer/addrs/ttl/issued_at）
    let mut tampered = announce_register(&kp, &addrs, 60, T0);
    tampered.ttl_secs = 3600;
    assert!(registry.register(&tampered, T0).is_err());

    // namespace 隔离：其他租户查不到 offer 帧
    let other = registry.query(&Query {
        namespace: "other/ns".to_string(),
        peer_id: Vec::new(),
    });
    assert!(other.peers.is_empty());
}
