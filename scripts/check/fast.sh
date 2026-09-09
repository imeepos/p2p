#!/usr/bin/env bash
# 分层快门禁 make check-fast：便宜门禁全跑（实测各 ≤2s），重门禁按受影响域裁剪。
# 判定：scripts/check/affected.sh（git 变更集 → crate 反向依赖闭包），本脚本只编排。
# 裁剪规则：
#   rust 域（FULL_RUST）→ clippy.sh/test.sh 全 workspace；否则按 AFFECTED_CRATES 闭包
#     逐点 clippy/test；rust 零变更 → SKIP（可观测输出，不算失败）
#   tauri 域 → clippy + gui-tauri.sh（FULL_ALL 时 clippy.sh 已含 tauri 段，不重复）
#   gui 域 → gui.sh；零变更 SKIP
# 便宜门禁（gate-tests/version/fmt/line-limit/panic-hygiene/cli-parity/ai-docs-sync）
#   恒跑：单步 ≤2s，裁剪它们省不了时间，漏跑只会造假绿。
# 红线：check-fast 只做"裁剪该跑什么"，不伪造任何绿；合并进 main 前、
#   release-check、CI 一律跑全量 make check。干净树 → 全 SKIP。
# 测试钩子：CHECK_ROOT 覆盖仓库根（scripts/check/tests/affected-fast.sh 夹具驱动）
# 自测：scripts/check/tests/affected-fast.sh
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "check-fast: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}
if command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER=sccache # 编译缓存（存在才启用，CI 无 sccache 时零影响）
fi

ROOT="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT"

# affected.sh stdout 仅 KEY=VALUE（值已白名单约束），stderr 为人读信号
eval "$(bash scripts/check/affected.sh)"

FAILS=0
step() { # step <名称> <命令...>：失败累计不中断，一次看全所有红
  local name="$1"
  shift
  echo "check-fast: RUN  $name"
  if "$@"; then
    echo "check-fast: PASS $name"
  else
    echo "check-fast: FAIL $name" >&2
    FAILS=$((FAILS + 1))
  fi
}
skip() { echo "check-fast: SKIP $1（$2）"; }

# --- 便宜门禁：恒跑 ---
step gate-tests-release bash scripts/check/tests/release-gates.sh
step gate-tests-panic bash scripts/check/tests/panic-hygiene.sh
step gate-tests-parity bash scripts/check/tests/cli-parity.sh
step gate-tests-mock-ipc bash scripts/check/tests/mock-ipc-guards.sh
step gate-tests-tauri bash scripts/check/tests/src-tauri-gate.sh
step gate-tests-latest-json bash scripts/check/tests/make-latest-json.sh
step version-check bash scripts/check/version.sh
step fmt-check bash scripts/check/fmt.sh
step line-limit bash scripts/check/line-limit.sh

# --- rust 域：闭包裁剪或全量 ---
if [ "$FULL_RUST" = 1 ]; then
  step clippy bash scripts/check/clippy.sh
  step test bash scripts/check/test.sh
elif [ -n "$AFFECTED_CRATES" ]; then
  PKGS=()
  for c in $AFFECTED_CRATES; do PKGS+=(-p "$c"); done
  step clippy-closure cargo clippy "${PKGS[@]}" --all-targets -- -D warnings
  step test-closure cargo test "${PKGS[@]}"
else
  skip clippy "rust 无变更"
  skip test "rust 无变更"
fi

# --- tauri 域（独立 workspace，根 clippy/test 不覆盖） ---
if [ "$DOMAIN_TAURI" = 1 ]; then
  if [ "$FULL_ALL" = 0 ]; then
    # FULL_ALL=1 时 clippy.sh 已含 src-tauri clippy 段，不重复跑
    step clippy-tauri bash -c 'cd apps/gui/src-tauri && cargo clippy --all-targets -- -D warnings'
  fi
  step test-tauri bash scripts/check/gui-tauri.sh
else
  skip gui-tauri-check "src-tauri 无变更"
fi

# --- gui 域 ---
if [ "$DOMAIN_GUI" = 1 ]; then
  step gui-check bash scripts/check/gui.sh
else
  skip gui-check "apps/gui 无变更"
fi

# --- 便宜门禁（后半）：恒跑 ---
step panic-hygiene bash scripts/check/panic-hygiene.sh
step cli-parity bash scripts/check/cli-parity.sh
step ai-docs-sync bash scripts/check/ai-docs-sync.sh

if [ "$FAILS" -gt 0 ]; then
  echo "check-fast: FAIL 共 $FAILS 步失败（SKIP≠通过，合并前请跑全量 make check）" >&2
  exit 1
fi
echo "check-fast: PASS"
