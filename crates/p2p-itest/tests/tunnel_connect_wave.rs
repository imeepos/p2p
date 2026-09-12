//! W-TC 双节点真链路 itest：headless 访侧 connect 形态（LocalProxy +
//! TunnelOpener 装配镜像 apps/cli connect.rs，apps/cli 为独立 cargo 项目无
//! lib 目标，itest 以同源 crate 面直连运行时——非子进程）。断言：HTTP GET
//! 经 B 反代得 A 服务固定 body；WS 升级 101 + accept 校验 + 文本/二进制帧
//! 双向 echo；访侧审计落 served 终态。

mod tunnel_common;
mod tunnel_connect_common;

use std::time::Duration;

use p2p_tunnel::{Head, TunnelAuditOutcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tunnel_connect_common::{
    rig, spawn_http_service, spawn_ws_echo_service, start_visit_proxy, wait_visit_record,
    ws_accept, ws_recv_frame, ws_send_frame, HTTP_BODY, WS_TEXT,
};

/// 单步等待上限：本地 loopback 全链毫秒级，15s 为宽松护栏。
const STEP: Duration = Duration::from_secs(15);

/// 绿：HTTP GET 经 B 本地反代全链透传，响应头 200 + 固定 body；访侧审计落
/// served 终态且 target 为白名单字面量。
#[tokio::test]
async fn g1_http_get_through_connect_proxy() {
    let http_port = spawn_http_service().await;
    let rig = rig("g1", vec![format!("127.0.0.1:{http_port}")]).await;
    let (addr, ctx, _task) = start_visit_proxy(&rig, http_port).await;

    let mut sock = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET / HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(STEP, sock.read_to_end(&mut raw))
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(
        text.starts_with("HTTP/1.1 200 OK"),
        "响应头须 200，resp={text}"
    );
    assert!(text.ends_with(HTTP_BODY), "body 须为固定 body，resp={text}");

    let rec = wait_visit_record(&ctx, 1).await;
    assert_eq!(rec.outcome, TunnelAuditOutcome::Served);
    assert_eq!(rec.target, format!("127.0.0.1:{http_port}"));
    assert_eq!(rec.peer_id, rig.a_peer.to_string(), "审计 peer 为被访节点");
}

/// 绿：WS 经 B 本地反代升级成功（101 + RFC6455 accept 校验）+ 文本帧与二进制
/// 帧双向 echo 往返 + close 帧干净收口。
#[tokio::test]
async fn g2_ws_upgrade_and_bidirectional_echo() {
    let ws_port = spawn_ws_echo_service().await;
    let rig = rig("g2", vec![format!("127.0.0.1:{ws_port}")]).await;
    let (addr, ctx, _task) = start_visit_proxy(&rig, ws_port).await;

    let mut sock = TcpStream::connect(addr).await.unwrap();
    // RFC6455 §1.3 示例 key；accept 断言基准同源计算。
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let req = format!(
        "GET /ws HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    sock.write_all(req.as_bytes()).await.unwrap();
    let (raw, _) = tokio::time::timeout(STEP, p2p_tunnel::read_head(&mut sock))
        .await
        .unwrap()
        .unwrap();
    let head = Head::parse(&raw).unwrap();
    assert_eq!(
        head.first_line.split(' ').nth(1),
        Some("101"),
        "须 101 升级"
    );
    assert_eq!(
        head.header("sec-websocket-accept").map(str::to_string),
        Some(ws_accept(key)),
        "accept 须 RFC6455 口径"
    );

    // 文本帧双向 echo。
    ws_send_frame(&mut sock, 0x1, WS_TEXT.as_bytes())
        .await
        .unwrap();
    let (opcode, payload) = tokio::time::timeout(STEP, ws_recv_frame(&mut sock))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opcode, 0x1, "echo 须同 opcode 文本帧");
    assert_eq!(payload, WS_TEXT.as_bytes());

    // 二进制帧双向 echo（含高位字节，防隐式 UTF-8 通道）。
    let binary = vec![0u8, 1, 2, 250, 255];
    ws_send_frame(&mut sock, 0x2, &binary).await.unwrap();
    let (opcode, payload) = tokio::time::timeout(STEP, ws_recv_frame(&mut sock))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opcode, 0x2);
    assert_eq!(payload, binary);

    // close 帧干净收口（服务端回应 close 后两侧断开）。
    ws_send_frame(&mut sock, 0x8, &[]).await.unwrap();
    let (opcode, _) = tokio::time::timeout(STEP, ws_recv_frame(&mut sock))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opcode, 0x8, "close 须被回显");

    // 升级路径为双向对称泵：本地半关（FIN）是隧道侧收口前提，审计才落终态。
    sock.shutdown().await.unwrap();
    let rec = wait_visit_record(&ctx, 1).await;
    assert_eq!(rec.outcome, TunnelAuditOutcome::Served);
    assert_eq!(rec.target, format!("127.0.0.1:{ws_port}"));
}
