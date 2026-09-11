#!/usr/bin/env bash
# 全量测试：cargo test --workspace
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"

# sccache 编译缓存（存在才启用；跨分支/冷启动重编提速，CI 无 sccache 零影响）
if command -v sccache >/dev/null 2>&1; then export RUSTC_WRAPPER=sccache; fi
command -v cargo >/dev/null 2>&1 || {
  echo "test: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

# p2p-itest task-wave 台架依赖 acp-echo-stub，但 apps/acp-agent 被 exclude 在根
# workspace 外（根 Cargo.toml），cargo test --workspace 不会构建它：主树靠历史
# 构建残留侥幸绿，fresh worktree / fresh CI checkout 必红（2026-09-11 W1 绿1轮
# 实锤，6/6 用例 0.09s 秒红于 stub 缺失 panic）。仿 cli-parity.sh 自保障 p2pctl
# 的先例：门禁自己保障夹具二进制；缺失即建（cargo 增量缓存，二次近零开销）。
STUB="apps/acp-agent/target/debug/acp-echo-stub"
if [ ! -x "$STUB" ]; then
  echo "test: 构建 itest 夹具 acp-echo-stub（apps/acp-agent 独立 workspace）"
  (cd apps/acp-agent && cargo build --bin acp-echo-stub) || {
    echo "test: FAIL acp-echo-stub 构建失败，p2p-itest task-wave 必红" >&2
    exit 1
  }
fi

cargo test --workspace
echo "test: PASS"
