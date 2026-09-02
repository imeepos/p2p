//! 地址观测（design §7.2）：UDP 反射协议，学习 NAT 映射后的外部地址。
//!
//! 约束：冻结的 SecureConn 不携带对端 socket 地址、rendezvous Response 无观测
//! 字段，故观测以独立 UDP 反射口实现（bootstrap 角色节点经
//! observation_port 启用反射器；节点向 observation_addrs 发探测学习自身
//! 外部地址）。反射地址与 QUIC/TCP 端口合并注册，跨网可拨；对称 NAT 的
//! 端口漂移场景由降级链的中继兜底，属已知边界。

use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

use p2p_transport::TransportAddr;

/// 观测协议魔法与版本（payload 前缀）。
const OBS_MAGIC: &[u8; 4] = b"OBS1";
/// 观测请求应答超时。
const OBSERVE_TIMEOUT: Duration = Duration::from_secs(2);

/// 观测反射器：bootstrap 角色节点绑定 UDP 端口，
/// 对合法探测回包告知对方「我观测到你的地址是 X」。
pub async fn spawn_reflector(port: u16) -> io::Result<SocketAddr> {
    let sock = UdpSocket::bind(SocketAddr::new(std::net::IpAddr::from([0, 0, 0, 0]), port)).await?;
    let local = pretty_local(sock.local_addr()?);
    tokio::spawn(async move {
        let mut buf = [0u8; 96];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) if n >= OBS_MAGIC.len() && buf[..OBS_MAGIC.len()] == *OBS_MAGIC => {
                    let mut resp = Vec::with_capacity(4 + 21);
                    resp.extend_from_slice(OBS_MAGIC);
                    resp.extend_from_slice(from.to_string().as_bytes());
                    let _ = sock.send_to(&resp, from).await;
                }
                Ok(_) => continue,
                Err(e) => {
                    tracing::warn!(error = %e, "observation reflector recv failed");
                    return;
                }
            }
        }
    });
    Ok(local)
}

/// 绑定 0.0.0.0 时报告地址换成 127.0.0.1（与 listen_addrs 约定一致）。
fn pretty_local(addr: SocketAddr) -> SocketAddr {
    if addr.ip().is_unspecified() {
        SocketAddr::new(std::net::IpAddr::from([127, 0, 0, 1]), addr.port())
    } else {
        addr
    }
}

/// 请求端：向观测口发探测，学习自身在公网侧的映射地址。
pub async fn observe_external(observation_addr: SocketAddr) -> io::Result<SocketAddr> {
    let sock = UdpSocket::bind(SocketAddr::new(std::net::IpAddr::from([0, 0, 0, 0]), 0)).await?;
    sock.send_to(OBS_MAGIC, observation_addr).await?;
    let mut buf = [0u8; 96];
    let (n, from) = tokio::time::timeout(OBSERVE_TIMEOUT, sock.recv_from(&mut buf))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "observation timed out"))??;
    if from != observation_addr || n < OBS_MAGIC.len() || buf[..OBS_MAGIC.len()] != *OBS_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "observation reply from unexpected source",
        ));
    }
    let text = std::str::from_utf8(&buf[OBS_MAGIC.len()..n])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad observation reply"))?;
    text.parse::<SocketAddr>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad observed addr"))
}

/// 逐个观测口尝试，取首个成功观测；全部失败返回空（调用方回退监听地址注册）。
pub async fn observe_external_addrs(observation_addrs: &[String]) -> Vec<SocketAddr> {
    let mut learned = Vec::new();
    for s in observation_addrs {
        let Ok(addr) = s.parse::<SocketAddr>() else {
            tracing::warn!(entry = %s, "malformed observation addr ignored");
            continue;
        };
        match observe_external(addr).await {
            Ok(observed) => {
                tracing::info!(%observed, via = %addr, "external address observed");
                learned.push(observed);
                break;
            }
            Err(e) => tracing::warn!(via = %addr, error = %e, "observation failed"),
        }
    }
    learned
}

/// 观测 IP × 本端 QUIC/TCP 端口 = 跨网可拨地址（去重；观测在前）。
pub fn observed_transport_addrs(
    observed: &[SocketAddr],
    listen_addrs: &[TransportAddr],
) -> Vec<TransportAddr> {
    let Some(mapped) = observed.first() else {
        return Vec::new();
    };
    let ip = mapped.ip();
    let mut out: Vec<TransportAddr> = Vec::new();
    for addr in listen_addrs {
        let candidate = match addr {
            TransportAddr::Quic { port, .. } => TransportAddr::Quic { ip, port: *port },
            TransportAddr::Tcp { port, .. } => TransportAddr::Tcp { ip, port: *port },
        };
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// 观测地址 + 监听地址合并为本端可注册地址（观测在前，跨网优先；去重）。
/// 观测 IP 配本端 QUIC/TCP 端口：cone NAT（含端口保持）下即为可拨公网地址。
pub fn merge_observed_with_listen(
    observed: Option<SocketAddr>,
    listen_addrs: &[TransportAddr],
) -> Vec<TransportAddr> {
    let mut out: Vec<TransportAddr> = Vec::new();
    let mut push = |addr: TransportAddr| {
        if !out.contains(&addr) {
            out.push(addr);
        }
    };
    if let Some(observed) = observed {
        for addr in listen_addrs {
            match addr {
                TransportAddr::Quic { port, .. } => push(TransportAddr::Quic {
                    ip: observed.ip(),
                    port: *port,
                }),
                TransportAddr::Tcp { port, .. } => push(TransportAddr::Tcp {
                    ip: observed.ip(),
                    port: *port,
                }),
            }
        }
    }
    for addr in listen_addrs {
        push(addr.clone());
    }
    p2p_swarm::filter_loopback(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reflector_reports_requester_observed_addr() {
        let local = spawn_reflector(0).await.expect("bind reflector");
        let observed = observe_external(local).await.expect("observe");
        // 请求端 socket 的本端地址即反射器看到的观测地址（loopback 无 NAT）
        assert_eq!(observed.ip().to_string(), "127.0.0.1");
        assert!(observed.port() > 0);
    }

    #[test]
    fn merge_puts_observed_first_and_dedups() {
        let listen = vec![
            TransportAddr::Quic {
                ip: "127.0.0.1".parse().unwrap(),
                port: 4000,
            },
            TransportAddr::Tcp {
                ip: "127.0.0.1".parse().unwrap(),
                port: 4001,
            },
        ];
        let observed: SocketAddr = "203.0.113.7:45001".parse().unwrap();
        let merged = merge_observed_with_listen(Some(observed), &listen);
        assert_eq!(
            merged[0],
            TransportAddr::Quic {
                ip: "203.0.113.7".parse().unwrap(),
                port: 4000
            }
        );
        // 全局观测存在时 loopback 监听地址被过滤（E3：远端拨 loopback 必拒）
        assert_eq!(merged.len(), 2, "observed x2 kept, loopback listen dropped");
        let merged = merge_observed_with_listen(None, &listen);
        assert_eq!(merged.len(), 2, "no observation keeps listen only");
    }
}
