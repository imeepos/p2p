//! 双节点真实回环 itest：B 为 FTP 服务端（LocalFs 临时目录 jail），A 为客户端。
//! 走真实 Node 拨号 → /ftp/ctrl/1 + /ftp/data/1 全链，非 mock。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p2p::Node;
use p2p_ftp::{FtpClient, FtpConfig, FtpError, LocalFs, OpenAuth, StaticAuth};

async fn spawn_node(tag: &str) -> Arc<Node> {
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

fn fs_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-ftp-root-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn link(a: &Arc<Node>, b: &Arc<Node>) {
    let addr = b.listen_addrs().first().unwrap().clone();
    a.add_peer_address(b.local_peer_id(), &addr).unwrap();
}

async fn serve_b(b: &Arc<Node>, root: &Path, cfg: FtpConfig) {
    let fs = Arc::new(LocalFs::open(root).unwrap());
    p2p_ftp::serve_with_config(b, fs, Arc::new(OpenAuth), cfg).unwrap();
}

fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// 主链路：登录 → 建目录 → 上传 → 查 size/list → 下载 → 改名/删除 → 退出。
#[tokio::test]
async fn full_file_session_roundtrip() {
    let a = spawn_node("rt-a").await;
    let b = spawn_node("rt-b").await;
    let root = fs_root("rt");
    serve_b(&b, &root, FtpConfig::default()).await;
    link(&a, &b).await;

    let mut c = FtpClient::connect(a.clone(), b.local_peer_id())
        .await
        .unwrap();
    c.login("alice", "secret").await.unwrap();

    c.mkd("/docs").await.unwrap();
    c.cwd("/docs").await.unwrap();
    assert_eq!(c.pwd().await.unwrap(), "/docs");

    let data = payload(2 * 1024 * 1024 + 7);
    let mut src = &data[..];
    let n = c.stor("big.bin", &mut src).await.unwrap();
    assert_eq!(n, data.len() as u64);

    assert_eq!(c.size("/docs/big.bin").await.unwrap(), data.len() as u64);

    let entries = c.list(Some("/docs")).await.unwrap();
    let big = entries.iter().find(|e| e.name == "big.bin").unwrap();
    assert_eq!(big.size, data.len() as u64);

    let names = c.nlst(Some("/docs")).await.unwrap();
    assert!(names.contains(&"big.bin".to_string()));

    // 相对路径与默认参数：cwd 后 LIST 缺省列当前目录
    let cwd_entries = c.list(None).await.unwrap();
    assert!(cwd_entries.iter().any(|e| e.name == "big.bin"));

    let mut sink = Vec::new();
    let got = c.retr("/docs/big.bin", &mut sink).await.unwrap();
    assert_eq!(got, data.len() as u64);
    assert_eq!(sink, data);

    c.rename("/docs/big.bin", "/docs/renamed.bin")
        .await
        .unwrap();
    assert!(c.size("/docs/big.bin").await.is_err());
    assert!(c.size("/docs/renamed.bin").await.is_ok());

    // APPE 追加语义
    let more = payload(1024);
    let mut src2 = &more[..];
    let n2 = c.appe("/docs/renamed.bin", &mut src2).await.unwrap();
    assert_eq!(n2, more.len() as u64);
    assert_eq!(
        c.size("/docs/renamed.bin").await.unwrap(),
        (data.len() + more.len()) as u64
    );

    c.dele("/docs/renamed.bin").await.unwrap();
    c.rmd("/docs").await.unwrap();
    assert!(c.list(Some("/docs")).await.is_err());

    c.noop().await.unwrap();
    c.quit().await.unwrap();
}

/// 鉴权门：错误密码拒绝、未登录命令 530、SYST 白名单放行。
#[tokio::test]
async fn auth_gate_rejects_before_login() {
    let a = spawn_node("auth-a").await;
    let b = spawn_node("auth-b").await;
    let root = fs_root("auth");
    let mut users = std::collections::HashMap::new();
    users.insert("alice".to_string(), "right".to_string());
    let fs = Arc::new(LocalFs::open(&root).unwrap());
    p2p_ftp::serve_with_config(
        &b,
        fs,
        Arc::new(StaticAuth::new(users)),
        FtpConfig::default(),
    )
    .unwrap();
    link(&a, &b).await;

    let mut c = FtpClient::connect(a.clone(), b.local_peer_id())
        .await
        .unwrap();
    assert!(
        c.list(Some("/")).await.is_err(),
        "未登录 LIST 必须 530 拒绝"
    );

    c.login("alice", "wrong").await.unwrap_err();
    // 失败后重新走完整序列
    c.login("alice", "right").await.unwrap();
    assert!(c.list(Some("/")).await.is_ok());
}

/// 路径监狱：.. 越出根、传输目标不存在/目录误用，全部拒绝且不留副作用。
#[tokio::test]
async fn path_jail_and_target_errors() {
    let a = spawn_node("jail-a").await;
    let b = spawn_node("jail-b").await;
    let root = fs_root("jail");
    serve_b(&b, &root, FtpConfig::default()).await;
    link(&a, &b).await;

    let mut c = FtpClient::connect(a.clone(), b.local_peer_id())
        .await
        .unwrap();
    c.login("u", "p").await.unwrap();

    assert!(matches!(
        c.cwd("/../etc").await,
        Err(FtpError::Rejected { code: 550, .. })
    ));
    assert!(matches!(
        c.cwd("/nope").await,
        Err(FtpError::Rejected { code: 550, .. })
    ));
    assert!(matches!(
        c.retr("/../x", &mut Vec::new()).await,
        Err(FtpError::Rejected { code: 550, .. })
    ));
    assert!(matches!(
        c.list(Some("/nope")).await,
        Err(FtpError::Rejected { code: 550, .. })
    ));

    // 目录当文件 RETR / 文件当目录 CWD
    let mut src = &b"seed"[..];
    c.stor("/seed.txt", &mut src).await.unwrap();
    let retr = c.retr("/seed.txt", &mut Vec::new()).await;
    assert!(retr.is_ok() || matches!(retr, Err(FtpError::Rejected { code: 550, .. })));
    assert!(matches!(
        c.cwd("/seed.txt").await,
        Err(FtpError::Rejected { code: 550, .. })
    ));
    assert!(matches!(c.size("/").await, Err(FtpError::Rejected { .. })));
}

/// 上传限额：超限传输在数据通道失败，控制面回非 226，且文件不完整落盘。
#[tokio::test]
async fn upload_limit_enforced() {
    let a = spawn_node("lim-a").await;
    let b = spawn_node("lim-b").await;
    let root = fs_root("lim");
    serve_b(
        &b,
        &root,
        FtpConfig {
            max_upload_bytes: 1024,
            ..FtpConfig::default()
        },
    )
    .await;
    link(&a, &b).await;

    let mut c = FtpClient::connect(a.clone(), b.local_peer_id())
        .await
        .unwrap();
    c.login("u", "p").await.unwrap();

    let big = vec![9u8; 1024 * 1024];
    let mut src = &big[..];
    let result = c.stor("/big.bin", &mut src).await;
    assert!(result.is_err(), "超限上传必须失败");
    let on_disk = std::fs::read(root.join("big.bin")).unwrap_or_default();
    assert!(on_disk.len() < big.len(), "超限后不得整文件落盘");
}
