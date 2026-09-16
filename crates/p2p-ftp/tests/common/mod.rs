//! 双节点回环 itest 共享工具：真实 Node 实例 + FTP 服务端装配。
//! 断言允许 unwrap/expect（tests 目录豁免 panic-hygiene）。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p2p::Node;
use p2p_ftp::{FtpConfig, LocalFs, OpenAuth};

pub async fn spawn_node(tag: &str) -> Arc<Node> {
    let dir = std::env::temp_dir().join(format!("p2p-ftp-itest-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Arc::new(
        Node::builder()
            .mdns(false)
            .quic_port(0)
            .tcp_port(0)
            .data_dir(dir.join("node"))
            .build()
            .await
            .unwrap(),
    )
}

pub fn fs_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-ftp-root-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub async fn link(a: &Arc<Node>, b: &Arc<Node>) {
    let addr = b.listen_addrs().first().unwrap().clone();
    a.add_peer_address(b.local_peer_id(), &addr).unwrap();
}

pub async fn serve_b(b: &Arc<Node>, root: &Path, cfg: FtpConfig) {
    let fs = Arc::new(LocalFs::open(root).unwrap());
    p2p_ftp::serve_with_config(b, fs, Arc::new(OpenAuth), cfg).unwrap();
}

pub fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}
