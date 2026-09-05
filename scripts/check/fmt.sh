#!/usr/bin/env bash
# 格式检查：cargo fmt --check（只读，不改动文件）
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "fmt: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}

cd "$(dirname "${BASH_SOURCE[0]}")/../.."
cargo fmt --check
echo "fmt: PASS"
