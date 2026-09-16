//! vdrive 波共用：双真 Node 装配 + 手写 HTTP/1.1 客户端（Connection: close）。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::Node;
use p2p_identity::PeerId;
use p2p_vdrive::{LocalFs, VDriveClient};

pub const STEP: Duration = Duration::from_secs(10);

pub struct Rig {
    pub host: Arc<Node>,
    pub guest: Arc<Node>,
    pub peer: PeerId,
    pub client: Arc<VDriveClient>,
    pub root: PathBuf,
    /// 持有根目录生命周期：目录随 Rig drop 而删（越狱校验依赖根存在）。
    pub _tmp: tempfile::TempDir,
}

/// 宿主发布 LocalFs(tempdir)，访客直连并持有 VDriveClient。
pub async fn rig(name: &str) -> Rig {
    let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{name}: tempdir: {e}"));
    let fs: Arc<dyn p2p_vdrive::FsBackend> = Arc::new(
        LocalFs::open(tmp.path())
            .await
            .unwrap_or_else(|e| panic!("{name}: localfs: {e}")),
    );
    let host = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(tmp.path().join("id-host"))
            .build()
            .await
            .unwrap_or_else(|e| panic!("{name}: host node: {e}")),
    );
    p2p_vdrive::serve(&host, fs).expect("serve vdrive");
    let guest = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(tmp.path().join("id-guest"))
            .build()
            .await
            .unwrap_or_else(|e| panic!("{name}: guest node: {e}")),
    );
    let peer = host.local_peer_id();
    for addr in host.listen_addrs() {
        guest.add_peer_address(peer, &addr).expect("add addr");
    }
    guest.connect(peer).await.expect("guest connect");
    let client = VDriveClient::new(Arc::clone(&guest), peer).expect("client");
    Rig {
        host,
        guest,
        peer,
        client: Arc::new(client),
        root: tmp.path().to_path_buf(),
        _tmp: tmp,
    }
}

/// 手写 HTTP/1.1 往返：恒 Connection: close，读到 EOF 收完整应答。
pub async fn http(
    addr: SocketAddr,
    method: &str,
    target: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let mut tcp = tokio::net::TcpStream::connect(addr).await.expect("dav connect");
    let mut head = format!("{method} {target} HTTP/1.1\r\nHost: bridge\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    let chunked = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("Transfer-Encoding"));
    if !chunked {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Connection: close\r\n\r\n");
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tcp.write_all(head.as_bytes()).await.expect("head write");
    tcp.write_all(body).await.expect("body write");
    tcp.shutdown().await.ok();
    let mut buf = Vec::new();
    tokio::time::timeout(STEP, tcp.read_to_end(&mut buf))
        .await
        .expect("response timeout")
        .expect("response read");
    parse_response(&buf)
}

fn parse_response(buf: &[u8]) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let sep = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response head terminator");
    let head = String::from_utf8_lossy(&buf[..sep]);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().expect("status line");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("bad status line: {status_line}"));
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let body = buf[sep + 4..].to_vec();
    (status, headers, body)
}

pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}
