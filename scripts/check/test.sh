#!/usr/bin/env bash
# 全量测试：cargo test --workspace
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "test: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}

cd "$(dirname "${BASH_SOURCE[0]}")/../.."
cargo test --workspace
echo "test: PASS"
