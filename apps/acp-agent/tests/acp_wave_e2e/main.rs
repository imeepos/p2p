//! ACP7 波次 E2E（设计 §7 故障矩阵逐行断言）：stub / 真 dsh 双模式集成测试。
//!
//! - stub 模式 hermetic：普通 `cargo test --test acp_wave_e2e` 即跑
//!   （子进程用 acp-echo-stub 桩，经 CARGO_BIN_EXE 定位，共享 tests/common 台架）。
//! - 真 dsh 模式：real_dsh 子模块用例全部 `#[ignore]`，验收以
//!   `cargo test --test acp_wave_e2e -- --ignored --test-threads 1` 单独跑；
//!   `dsh --profile acp` 不可用（spawn 失败 / initialize 无应答）时直写 stderr
//!   打印 `SKIP:` 信号后结束用例，绝不假绿。argv 可用环境变量 ACP_E2E_REAL_DSH
//!   覆盖（空格切分，默认 "dsh --profile acp"）。
//!
//! 场景链（任务卡 A）：①未授权拒绝 ②授权握手+透传 roundtrip ③prompt 应答流
//! ④权限 ask→批准→一次性 grant ⑤断流续连补放 ⑥窗口过期退出阶梯 ⑦连接门禁超限
//! ⑧mcpServers 剥离/白名单。

#[path = "../common/mod.rs"]
mod common;

mod real_dsh;
mod stub_resilience;
mod stub_security;
mod stub_session;

use std::io::Write as _;
use std::time::Duration;

use acp_agent::{AuditEvent, CaptureAudit};
use p2p::BoxedStream;

/// libtest 会捕获 print!/eprint! 宏输出，直写 stderr 才能无条件留下 SKIP 信号。
pub(crate) fn skip_signal(reason: &str) {
    let _ = std::io::stderr().write_all(
        format!(
            "SKIP: real dsh unavailable: {reason}
"
        )
        .as_bytes(),
    );
}

/// 带时限读一行：EOF 与超时统一返回 None（真链路可用性判定依赖此语义）。
pub(crate) async fn line_within(stream: &mut BoxedStream, secs: u64) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(secs), common::read_line(stream))
        .await
        .ok()
        .flatten()
}

/// 条件等待审计事件：替代盲睡，超时带全量快照便于排障。
pub(crate) async fn wait_audit(
    audit: &CaptureAudit,
    pred: impl Fn(&AuditEvent) -> bool + Copy,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if audit.contains(pred) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "audit wait timed out: {:?}",
            audit.snapshot(),
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
