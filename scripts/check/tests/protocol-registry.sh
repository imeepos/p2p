#!/usr/bin/env bash
# protocol-registry 门禁自测挂接：驱动主脚本 --self-test（mktemp 沙箱红绿夹具）
# 主脚本内置夹具与断言；本层保证 gate-tests 链路调用的是同一入口（防双实现漂移）
set -u
set -o pipefail

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$CHECK_DIR/protocol-registry.sh"

if [ ! -f "$GATE" ]; then
  echo "protocol-registry 自测：FAIL 主脚本缺失: $GATE" >&2
  exit 1
fi
if ! bash -n "$GATE"; then
  echo "protocol-registry 自测：FAIL 主脚本语法错误" >&2
  exit 1
fi
if ! bash "$GATE" --self-test; then
  echo "protocol-registry 自测：FAIL" >&2
  exit 1
fi
echo "protocol-registry 自测：PASS"
