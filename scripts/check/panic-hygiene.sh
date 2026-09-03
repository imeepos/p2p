#!/usr/bin/env bash
# panic 卫生门禁：crates/ 非测试代码禁止 .unwrap() / .expect( / panic!(
# 范围：crates/ 全部 .rs；排除 tests|examples|benches 目录、tests.rs（测试模块文件约定）、
#   #[cfg(test)] 模块体（按花括号配对近似界定，含同行与独立行 mod 声明两种形态）
# 豁免：scripts/check/panic-hygiene-exempt.txt（crate 名 + 一行理由），PANIC_HYGIENE_EXEMPT 可覆盖；
#   受保护 crate（PROTECTED，已清零范围）禁止出现在豁免清单，违者直接红
# 测试钩子：CHECK_ROOT 覆盖仓库根（scripts/check/tests/panic-hygiene.sh 夹具驱动）
# 局限：文本扫描——索引越界/算术溢出/字符串内花括号等不在覆盖范围
set -uo pipefail

ROOT="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
EXEMPT_FILE="${PANIC_HYGIENE_EXEMPT:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/panic-hygiene-exempt.txt}"
CRATES_DIR="$ROOT/crates"

# 已完成清零、门禁直接管辖的六 crate：不得进豁免清单（防门禁自宽）。
# p2p/p2p-log 系 E8-H3 收缩（此前豁免，清零后转保护）
PROTECTED=" p2p-protocol p2p-discovery p2p-identity p2p-security p2p p2p-log "

if [ ! -d "$CRATES_DIR" ]; then
  echo "panic-hygiene: FAIL 找不到目录 $CRATES_DIR" >&2
  exit 1
fi
if [ ! -f "$EXEMPT_FILE" ]; then
  echo "panic-hygiene: FAIL 豁免清单缺失: $EXEMPT_FILE" >&2
  exit 1
fi

# 豁免清单解析：空行与 # 注释跳过；首 token 为 crate 名，其余为理由（必填）
EXEMPTS=" "
manifest_ok=1
while read -r name reason; do
  case "$name" in "" | "#"*) continue ;; esac
  if [ -z "$reason" ]; then
    echo "panic-hygiene: FAIL 豁免条目缺理由: $name" >&2
    manifest_ok=0
  fi
  case "$PROTECTED" in *" $name "*)
    echo "panic-hygiene: FAIL 受保护 crate 禁止加入豁免清单: $name" >&2
    manifest_ok=0
  esac
  EXEMPTS+="$name "
done < "$EXEMPT_FILE"
if [ "$manifest_ok" -ne 1 ]; then
  echo "panic-hygiene: FAIL 豁免清单校验未过" >&2
  exit 1
fi

is_exempt() {
  case "$EXEMPTS" in *" $1 "*) return 0 ;; *) return 1 ;; esac
}

# 打印违规行（file:line: 内容）；#[cfg(test)] 模块体整段跳过
scan_file() {
  awk -v rel="$2" '
    /^[[:space:]]*#\[cfg\(test\)\]/ {
      rest = $0
      sub(/^[[:space:]]*#\[cfg\(test\)\]/, "", rest)
      if (rest ~ /^[[:space:]]*(pub([^{}]*)[[:space:]]+)?mod[[:space:]]+[A-Za-z_]/) {
        pending = 0
        depth = gsub(/{/, "{", rest) - gsub(/}/, "}", rest)
        if (depth > 0) in_test = 1
        next
      }
      if (rest ~ /^[[:space:]]*$/) { pending = 1; next }
      next
    }
    pending {
      if ($0 ~ /^[[:space:]]*(pub([^{}]*)[[:space:]]+)?mod[[:space:]]+[A-Za-z_]/) {
        pending = 0
        depth = gsub(/{/, "{") - gsub(/}/, "}")
        if (depth > 0) in_test = 1
        next
      }
      if ($0 ~ /^[[:space:]]*(\/\/|$)/) next
      pending = 0
    }
    in_test {
      depth += gsub(/{/, "{") - gsub(/}/, "}")
      if (depth <= 0) in_test = 0
      next
    }
    /\.unwrap\(\)|\.expect\(|panic!\(/ { print rel ":" FNR ": " $0 }
  ' "$1"
}

status=0
scanned=0
bad_files=0
while IFS= read -r -d '' file; do
  rel=${file#"$ROOT"/}
  crate=${rel#crates/}; crate=${crate%%/*}
  is_exempt "$crate" && continue
  scanned=$((scanned + 1))
  hits=$(scan_file "$file" "$rel")
  if [ -n "$hits" ]; then
    bad_files=$((bad_files + 1))
    status=1
    printf '%s\n' "$hits" >&2
  fi
done < <(find "$CRATES_DIR" -type f -name '*.rs' \
  ! -path '*/tests/*' ! -path '*/examples/*' ! -path '*/benches/*' ! -name 'tests.rs' -print0)

if [ "$status" -ne 0 ]; then
  echo "panic-hygiene: FAIL $bad_files 个文件在非测试路径存在 unwrap/expect/panic" >&2
  exit 1
fi
if [ "$scanned" -eq 0 ]; then
  echo "panic-hygiene: FAIL 未扫描到任何 .rs 文件（目录或排除规则异常）" >&2
  exit 1
fi
exempt_count=$(printf '%s' "$EXEMPTS" | wc -w | tr -d ' ')
echo "panic-hygiene: PASS（扫描 $scanned 个文件非测试路径零 unwrap/expect/panic，豁免 $exempt_count 个 crate）"
