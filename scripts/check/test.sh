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
cargo test --workspace
echo "test: PASS"
