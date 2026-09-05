#!/usr/bin/env bash
# 格式检查：cargo fmt --check（只读，不改动文件）
# 覆盖两域：根 workspace + apps/gui/src-tauri 独立 workspace。后者被根
# Cargo.toml exclude，根 fmt 扫不到（2026-09-05 chat.rs 漂移随 PR 漏网实锤）。
# rustfmt 纯语法解析不编译依赖，src-tauri 段无需系统库，CI ubuntu 照常真跑。
# 测试钩子：CHECK_ROOT 覆盖仓库根（scripts/check/tests/src-tauri-gate.sh 夹具驱动）
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "fmt: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}

ROOT="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT"
cargo fmt --check || {
  echo "fmt: FAIL 根 workspace 存在格式漂移（上为 rustfmt diff，跑 make fmt 修复）" >&2
  exit 1
}

if [ -d "$ROOT/apps/gui/src-tauri" ]; then
  (cd "$ROOT/apps/gui/src-tauri"
    cargo fmt --check || {
      echo "fmt: FAIL src-tauri 存在格式漂移（上为 rustfmt diff，cd apps/gui/src-tauri && cargo fmt 修复）" >&2
      exit 1
    })
  echo "fmt: src-tauri 段 PASS"
fi
echo "fmt: PASS"
