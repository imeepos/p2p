//! 网络硬盘协议波（/vdrive/fs/1，specs/vdrive.md 验收）：真双 Node QUIC
//! 回环全链——操作面语义、大文件分块回读、监狱越狱拒绝、只读策略、
//! unit 操作失败结构化上抛（防静默吞错回归）。

mod vdrive_common;

use std::sync::Arc;

use p2p_vdrive::server::{AccessPolicy, OpClass};
use p2p_vdrive::{EntryKind, ErrorKind, FsBackend, VDriveClient, MAX_CHUNK};
use vdrive_common::rig;

/// 拒写策略（协议面策略裁决的 E2E 断言载体）。
struct DenyWrite;

#[async_trait::async_trait]
impl AccessPolicy for DenyWrite {
    async fn allow(&self, _: &p2p_identity::PeerId, class: OpClass) -> bool {
        class == OpClass::Read
    }
}

#[tokio::test]
async fn fs_lifecycle_over_real_nodes() {
    let rig = rig("lifecycle").await;
    let c = &rig.client;

    let statfs = c.statfs().await.expect("statfs");
    assert_eq!(statfs.total_bytes, 0, "M1 容量不报告（0 = unknown）");

    c.mkdir("/docs").await.expect("mkdir");
    let err = c.mkdir("/docs").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "重复 mkdir 报 already_exists");

    let entry = c.create("/docs/hello.txt").await.expect("create");
    assert_eq!(entry.kind, EntryKind::File);
    assert_eq!(entry.size, 0);

    let payload = b"hello vdrive network disk".to_vec();
    let written = c.write("/docs/hello.txt", 0, &payload).await.expect("write");
    assert_eq!(written, payload.len() as u64);

    let st = c.stat("/docs/hello.txt").await.expect("stat");
    assert_eq!(st.size, payload.len() as u64);
    assert!(st.mtime > 0, "mtime 报告（LocalFs 系统语义）");

    let got = c.read("/docs/hello.txt", 0, MAX_CHUNK).await.expect("read");
    assert_eq!(got, payload);
    let tail = c.read("/docs/hello.txt", 6, MAX_CHUNK).await.expect("read tail");
    assert_eq!(tail, &payload[6..], "偏移读");
    let eof = c.read("/docs/hello.txt", 999, MAX_CHUNK).await.expect("read eof");
    assert!(eof.is_empty(), "越界偏移 = 短读 0（EOF 语义）");

    let entries = c.list("/docs").await.expect("list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "hello.txt");

    c.rename("/docs/hello.txt", "/docs/hi.txt").await.expect("rename");
    let err = c.stat("/docs/hello.txt").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound, "旧名消失");
    c.truncate("/docs/hi.txt", 4).await.expect("truncate");
    assert_eq!(c.stat("/docs/hi.txt").await.unwrap().size, 4);

    c.unlink("/docs/hi.txt").await.expect("unlink");
    c.rmdir("/docs").await.expect("空目录 rmdir 成功");
    let err = c.stat("/docs").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound, "目录已删");
    rig.host.shutdown();
    rig.guest.shutdown();
}

#[tokio::test]
async fn large_file_chunked_roundtrip() {
    let rig = rig("large").await;
    let c = &rig.client;
    let mut payload = vec![0u8; MAX_CHUNK as usize * 2 + 777];
    for (i, b) in payload.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    c.create("/big.bin").await.expect("create big");
    let mut offset = 0u64;
    while offset < payload.len() as u64 {
        let end = ((offset as usize) + MAX_CHUNK as usize).min(payload.len());
        let n = c
            .write("/big.bin", offset, &payload[offset as usize..end])
            .await
            .expect("chunk write");
        assert_eq!(n, (end - offset as usize) as u64);
        offset = end as u64;
    }
    assert_eq!(c.stat("/big.bin").await.unwrap().size, payload.len() as u64);
    let mut back = Vec::with_capacity(payload.len());
    let mut pos = 0u64;
    loop {
        let chunk = c.read("/big.bin", pos, MAX_CHUNK).await.expect("chunk read");
        if chunk.is_empty() {
            break;
        }
        pos += chunk.len() as u64;
        back.extend_from_slice(&chunk);
    }
    assert_eq!(back, payload, "分块回读全量且逐字节一致");
    rig.host.shutdown();
    rig.guest.shutdown();
}

#[tokio::test]
async fn jail_escape_rejected() {
    let rig = rig("jail").await;
    let c = &rig.client;
    let err = c.stat("/../etc/passwd").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidPath);
    let err = c.list("/a/../../").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidPath);
    let err = c.write("/../evil", 0, b"x").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidPath);
    rig.host.shutdown();
    rig.guest.shutdown();
}

#[tokio::test]
async fn write_policy_denied_but_read_allowed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let fs: Arc<dyn p2p_vdrive::FsBackend> =
        Arc::new(p2p_vdrive::LocalFs::open(tmp.path()).await.expect("localfs"));
    let host = Arc::new(
        p2p::Node::builder()
            .mdns(false)
            .data_dir(tmp.path().join("id-host"))
            .build()
            .await
            .expect("host"),
    );
    p2p_vdrive::serve_with_policy(&host, fs, Arc::new(DenyWrite)).expect("serve");
    let guest = Arc::new(
        p2p::Node::builder()
            .mdns(false)
            .data_dir(tmp.path().join("id-guest"))
            .build()
            .await
            .expect("guest"),
    );
    let peer = host.local_peer_id();
    for addr in host.listen_addrs() {
        guest.add_peer_address(peer, &addr).expect("add addr");
    }
    guest.connect(peer).await.expect("connect");
    let c = VDriveClient::new(guest.clone(), peer).expect("client");

    c.mkdir("/blocked").await.expect_err("策略拒绝写");
    c.statfs().await.expect("策略放行读");
    host.shutdown();
    guest.shutdown();
}

#[tokio::test]
async fn unit_error_wire_not_silently_swallowed() {
    // 回归：unit 操作失败必须结构化上抛（防静默吞错回潮）。
    let rig = rig("unit-err").await;
    let err = rig.client.rmdir("/no/such/dir").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound);
    let err = rig.client.unlink("/no/such/file").await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound);
    rig.host.shutdown();
    rig.guest.shutdown();
}
