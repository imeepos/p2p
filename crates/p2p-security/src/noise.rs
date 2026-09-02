//! Noise XX（TCP 场景）：握手 payload 携带 ed25519 公钥与签名。
//!
//! 身份绑定：签名覆盖本端 Noise X25519 静态公钥，攻击者无法在持有自己静态钥的
//! 同时冒充他人 ed25519 身份；对端 PeerId 一律从 payload 公钥推导，不采信自报。
//! 全握手受 deadline 约束（帧级 + 总时长双层），半开握手到期即断（安全审查 1 期 M3）。

use std::io;
use std::time::Duration;

use tokio::time::Instant as TokioInstant;

use async_trait::async_trait;
use snow::HandshakeState;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::SecurityUpgrade;
use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;

use crate::noise_stream::NoiseStream;

/// snow 参数：XX + 25519 + ChaChaPoly + SHA256
pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
/// 默认握手总时长上限：防 slowloris 半开握手占用（安全审查 1 期 M3）
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// 签名域分隔，防止握手签名被挪作他用
const SIGN_DOMAIN: &[u8] = b"p2p-noise-xx-v1";

/// TCP 场景的安全升级实现（XX 模式，双方交换静态钥）。
pub struct NoiseXx {
    handshake_timeout: Duration,
}

impl NoiseXx {
    pub fn new() -> Self {
        Self {
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }

    /// 覆盖握手时长上限（测试与特殊网络环境用）。
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }
}

impl Default for NoiseXx {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecurityUpgrade for NoiseXx {
    async fn outbound(
        &self,
        stream: BoxedStream,
        keypair: &Keypair,
        expected: Option<PeerId>,
    ) -> Result<(PeerId, BoxedStream), crate::SecurityError> {
        let deadline = TokioInstant::now() + self.handshake_timeout;
        let handshake = async {
            let mut stream = stream;
            let mut hs = build(keypair, true)?;
            send_frame(&mut stream, &mut hs, &[], deadline).await?;
            let payload = recv_frame(&mut stream, &mut hs, deadline).await?;
            let static_pub = hs
                .get_remote_static()
                .ok_or(crate::SecurityError::IdentityUnverified)?
                .to_vec();
            let peer = verify_payload(&payload, &static_pub)?;
            ensure_expected(expected, peer)?;
            send_frame(&mut stream, &mut hs, &make_payload(keypair), deadline).await?;
            let transport = hs.into_transport_mode().map_err(handshake_err)?;
            Ok((
                peer,
                Box::new(NoiseStream::new(stream, transport)) as BoxedStream,
            ))
        };
        match tokio::time::timeout_at(deadline, handshake).await {
            Ok(result) => result,
            Err(_) => Err(deadline_err()),
        }
    }

    async fn inbound(
        &self,
        stream: BoxedStream,
        keypair: &Keypair,
    ) -> Result<(PeerId, BoxedStream), crate::SecurityError> {
        let deadline = TokioInstant::now() + self.handshake_timeout;
        let handshake = async {
            let mut stream = stream;
            let mut hs = build(keypair, false)?;
            recv_frame(&mut stream, &mut hs, deadline).await?;
            send_frame(&mut stream, &mut hs, &make_payload(keypair), deadline).await?;
            let payload = recv_frame(&mut stream, &mut hs, deadline).await?;
            let static_pub = hs
                .get_remote_static()
                .ok_or(crate::SecurityError::IdentityUnverified)?
                .to_vec();
            let peer = verify_payload(&payload, &static_pub)?;
            let transport = hs.into_transport_mode().map_err(handshake_err)?;
            Ok((
                peer,
                Box::new(NoiseStream::new(stream, transport)) as BoxedStream,
            ))
        };
        match tokio::time::timeout_at(deadline, handshake).await {
            Ok(result) => result,
            Err(_) => Err(deadline_err()),
        }
    }
}

fn handshake_err(e: snow::Error) -> crate::SecurityError {
    crate::SecurityError::Handshake(format!("snow: {e}"))
}

fn io_err(e: io::Error) -> crate::SecurityError {
    crate::SecurityError::Handshake(format!("io: {e}"))
}

fn deadline_err() -> crate::SecurityError {
    crate::SecurityError::Handshake("handshake deadline exceeded".into())
}

/// ed25519 种子按 RFC 标准转换为 X25519 静态私钥（SHA512 前半 + clamp）
fn x25519_private(keypair: &Keypair) -> [u8; 32] {
    use sha2::{Digest, Sha512};
    let hash = Sha512::digest(keypair.to_seed_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash[..32]);
    key[0] &= 248;
    key[31] &= 127;
    key[31] |= 64;
    key
}

/// X25519 静态公钥：ed25519 公钥点转蒙哥马利坐标（同一标量乘基点的结果）
fn x25519_public(keypair: &Keypair) -> [u8; 32] {
    use ed25519_dalek::VerifyingKey;
    let vk = VerifyingKey::from_bytes(&keypair.public()).expect("ed25519 pubkey valid");
    *vk.to_montgomery().as_bytes()
}

fn build(keypair: &Keypair, initiator: bool) -> Result<HandshakeState, crate::SecurityError> {
    let params: snow::params::NoiseParams = NOISE_PATTERN.parse().map_err(handshake_err)?;
    let static_priv = x25519_private(keypair);
    let builder = snow::Builder::new(params).local_private_key(&static_priv);
    if initiator {
        builder.build_initiator().map_err(handshake_err)
    } else {
        builder.build_responder().map_err(handshake_err)
    }
}

/// payload = 32B ed25519 公钥 + 64B 签名（覆盖本端 X25519 静态公钥 + 域分隔）
fn make_payload(keypair: &Keypair) -> Vec<u8> {
    let mut msg = Vec::with_capacity(SIGN_DOMAIN.len() + 32);
    msg.extend_from_slice(SIGN_DOMAIN);
    msg.extend_from_slice(&x25519_public(keypair));
    let sig = keypair.sign(&msg);
    let mut payload = Vec::with_capacity(32 + 64);
    payload.extend_from_slice(&keypair.public());
    payload.extend_from_slice(&sig);
    payload
}

fn verify_payload(payload: &[u8], remote_static: &[u8]) -> Result<PeerId, crate::SecurityError> {
    if payload.len() != 96 {
        return Err(crate::SecurityError::Handshake(format!(
            "identity payload len {} != 96",
            payload.len()
        )));
    }
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&payload[..32]);
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&payload[32..]);
    let mut signed = Vec::with_capacity(SIGN_DOMAIN.len() + remote_static.len());
    signed.extend_from_slice(SIGN_DOMAIN);
    signed.extend_from_slice(remote_static);
    if !Keypair::verify(&pubkey, &signed, &sig) {
        return Err(crate::SecurityError::IdentityUnverified);
    }
    Ok(PeerId::from_public_key(&pubkey))
}

fn ensure_expected(expected: Option<PeerId>, actual: PeerId) -> Result<(), crate::SecurityError> {
    match expected {
        Some(exp) if exp != actual => Err(crate::SecurityError::PeerMismatch {
            expected: exp.to_string(),
            actual: actual.to_string(),
        }),
        _ => Ok(()),
    }
}

/// 握手帧与传输态同格式：[u16 帧长][密文]
/// 写缓冲需容纳最大 token 开销（XX msg2 = e32 + s48 + tag16），给足余量
const HANDSHAKE_FRAME_OVERHEAD: usize = 160;

/// 单步 IO 受 deadline 约束：慢读慢写都会触发握手超时
async fn io_step<T>(
    deadline: TokioInstant,
    op: impl std::future::Future<Output = io::Result<T>>,
) -> Result<T, crate::SecurityError> {
    match tokio::time::timeout_at(deadline, op).await {
        Ok(result) => result.map_err(io_err),
        Err(_) => Err(deadline_err()),
    }
}

async fn send_frame(
    stream: &mut BoxedStream,
    hs: &mut HandshakeState,
    payload: &[u8],
    deadline: TokioInstant,
) -> Result<(), crate::SecurityError> {
    let mut msg = vec![0u8; payload.len() + HANDSHAKE_FRAME_OVERHEAD];
    let n = hs.write_message(payload, &mut msg).map_err(handshake_err)?;
    io_step(deadline, stream.write_all(&(n as u16).to_be_bytes())).await?;
    io_step(deadline, stream.write_all(&msg[..n])).await?;
    io_step(deadline, stream.flush()).await
}

async fn recv_frame(
    stream: &mut BoxedStream,
    hs: &mut HandshakeState,
    deadline: TokioInstant,
) -> Result<Vec<u8>, crate::SecurityError> {
    let mut len_buf = [0u8; 2];
    io_step(deadline, stream.read_exact(&mut len_buf)).await?;
    let len = u16::from_be_bytes(len_buf) as usize;
    let mut msg = vec![0u8; len];
    io_step(deadline, stream.read_exact(&mut msg)).await?;
    let mut payload = vec![0u8; len];
    let n = hs.read_message(&msg, &mut payload).map_err(handshake_err)?;
    payload.truncate(n);
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn xx_handshake_binds_peer_identity() {
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let (a, b) = duplex(64 * 1024);
        let sa: BoxedStream = Box::new(a);
        let sb: BoxedStream = Box::new(b);

        let server = NoiseXx::new();
        let client = NoiseXx::new();
        let (server, client) = tokio::join!(
            server.inbound(sb, &bob),
            client.outbound(sa, &alice, Some(bob.peer_id()))
        );
        let (server_peer, mut server_stream) = server.expect("server handshake");
        let (client_peer, mut client_stream) = client.expect("client handshake");
        assert_eq!(server_peer, alice.peer_id(), "server sees client identity");
        assert_eq!(client_peer, bob.peer_id(), "client sees server identity");

        client_stream.write_all(b"secret").await.expect("write");
        client_stream.flush().await.expect("flush");
        let mut buf = [0u8; 6];
        server_stream.read_exact(&mut buf).await.expect("read");
        assert_eq!(&buf, b"secret");
    }

    #[tokio::test]
    async fn wrong_expected_peer_is_rejected() {
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let eve = Keypair::generate();
        let (a, b) = duplex(64 * 1024);
        let sa: BoxedStream = Box::new(a);
        let sb: BoxedStream = Box::new(b);

        let server = NoiseXx::new();
        let client = NoiseXx::new();
        let (server, client) = tokio::join!(
            server.inbound(sb, &bob),
            client.outbound(sa, &alice, Some(eve.peer_id()))
        );
        assert!(
            server.is_err() || client.is_err(),
            "expected mismatch must fail"
        );
    }
}
