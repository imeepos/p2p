#!/usr/bin/env bash
# clippy 门禁：全 workspace 全 target，警告一律当错误（-D warnings）
set -eu
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || {
  echo "clippy: cargo 不在 PATH（预期 $HOME/.cargo/bin）" >&2
  exit 127
}

cd "$(dirname "${BASH_SOURCE[0]}")/../.."
cargo clippy --workspace --all-targets -- -D warnings
echo "clippy: PASS"
