//! 底座流语义行为锁定探针（治理文档 docs/notes/2026-09-05-yamux-stream-semantics.md
//! 的可执行附件）：把「流级 shutdown 交付 FIN/EOF」与「连接死亡终结读侧」两条
//! 存活判据前提机械锁死——任一翻转即提示同步审视 pump 兜底与事件竞速设计。

mod common;

use std::time::Duration;

use tokio::io::AsyncReadExt;

use acp_console::dial::dial_and_handshake;

use common::*;

/// 断言：agent 侧流级 shutdown 会对端可见（FIN → EOF）。futures 默认
/// poll_shutdown 委托 poll_close，yamux CloseStream 帧照发——半关闭可用。
/// 若本测试失败（读不到 EOF），说明底座半关闭回归——pump 的重 kick 与
/// PeerDisconnected 竞速兜底就要按唯一存活判据重新审视。
#[tokio::test]
async fn stream_shutdown_delivers_eof_to_peer() {
    let rig = rig("halfclose", AgentMock::half_closing()).await;
    let (_, _, mut stream) = dial_and_handshake(&rig.console, rig.agent_peer, None, None)
        .await
        .unwrap();

    let mut buf = Vec::new();
    let probe = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut buf)).await;
    match probe {
        Ok(Ok(0)) => {} // 期望形态：干净 EOF
        Ok(other) => panic!("unexpected read outcome: {other:?}"),
        Err(_) => panic!("shutdown 后对端 5s 未收到 EOF：半关闭回归，存活判据失效"),
    }
    teardown(rig);
}

/// 对照面：连接级断开（真实 agent 死亡路径）读侧必须终结——EOF 或错误二选一，
/// 不允许永久 Pending（这是 pump 事件竞速兜底成立的前提）。
#[tokio::test]
async fn connection_death_terminates_stream_read() {
    let rig = rig("connloss", AgentMock::echo()).await;
    let (_, _, mut stream) = dial_and_handshake(&rig.console, rig.agent_peer, None, None)
        .await
        .unwrap();
    rig.agent.shutdown();

    let mut buf = Vec::new();
    let probe = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut buf)).await;
    match probe {
        Ok(Ok(_)) => {}  // EOF 形态
        Ok(Err(_)) => {} // 错误形态（BrokenPipe/Reset）
        Err(_) => panic!("连接死亡后读侧 5s 仍未终结：存活判据前提被破坏"),
    }
    teardown(rig);
}
