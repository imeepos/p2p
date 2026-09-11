#!/usr/bin/env bash
# 门禁脚本变量名炸弹守卫（2026-09-11 W1 回归固化）：
# bash 在多字节 locale（如 CI ubuntu 的 en_US.UTF-8）下，$var 后紧跟的非 ASCII
# 字符会被并进变量名一起解析（例如 name 后接全角左括号会被解析成一个叫
# 「name 加全角括号」的变量），set -u 下直接击杀脚本；C locale 不触发，
# 所以出现「本机绿、CI 三连红」。实锤路径：affected.sh 回退 err、
# affected-fast.sh / protocol-registry.sh 的断言 FAIL 行（红因被吞不可观测）。
# 规约：变量引用后紧跟非 ASCII 字节必须写作花括号形式。
# 本守卫按字节扫描（LC_ALL=C），与运行 locale 无关；命中即红并打印命中行。
set -u
set -o pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pattern='\$[A-Za-z_][A-Za-z0-9_]*[^[:print:][:space:]]'

hits=0
scanned=0
while IFS= read -r f; do
  scanned=$((scanned + 1))
  out="$(LC_ALL=C grep -nE "$pattern" "$f" 2>/dev/null || true)"
  if [ -n "$out" ]; then
    hits=$((hits + 1))
    echo "ascii-var-guard: FAIL $f" >&2
    printf '%s\n' "$out" >&2
  fi
done <<EOF
$(find "$DIR" -name '*.sh' -type f | sort)
EOF

echo "ascii-var-guard: scanned=$scanned hits=$hits"
[ "$hits" -eq 0 ]
