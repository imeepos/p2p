#!/usr/bin/env bash
# G-A2 双节点冒烟回归入口（W3 集成复用）：包装 src-tauri 的 smoke 集成测试。
# 用法：bash scripts/gui-smoke.sh [--quiet]；退出码即测试结果。
# 注意：会占用 mDNS 与本地随机端口，请勿与其他 smoke 并发运行。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

# 兜底清理历史 smoke 残留（前缀不过夜）；本次运行目录由测试自身管理
find "${TMPDIR:-/tmp}" -maxdepth 1 -name 'smoke_p2p_gui_*' -exec rm -rf {} + 2>/dev/null || true

cd "$ROOT/apps/gui/src-tauri"
echo "[gui-smoke] cargo test --test smoke (RUST_LOG=${RUST_LOG:-warn})"
exec env RUST_LOG="${RUST_LOG:-warn}" cargo test --test smoke -- --nocapture --test-threads=1
