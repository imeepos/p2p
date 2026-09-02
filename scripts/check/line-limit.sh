#!/usr/bin/env bash
# 行数红线：crates/ 下所有 .rs 文件 <= LINE_LIMIT（默认 300），超限列出文件与行数并 exit 1
# 豁免清单：环境变量 LINE_LIMIT_EXEMPT，空格分隔的仓库根相对路径，例：
#   LINE_LIMIT_EXEMPT="crates/p2p/src/legacy.rs" bash scripts/check/line-limit.sh
set -euo pipefail

LINE_LIMIT="${LINE_LIMIT:-300}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CRATES_DIR="$ROOT/crates"

if [ ! -d "$CRATES_DIR" ]; then
  echo "line-limit: 找不到目录 $CRATES_DIR" >&2
  exit 1
fi

# 空格分隔整体词精确匹配豁免；用 case 做 glob 匹配（test 的 = 不做通配）
is_exempt() {
  case " ${LINE_LIMIT_EXEMPT:-} " in
    *" $1 "*) return 0 ;;
    *) return 1 ;;
  esac
}

status=0
count=0
while IFS= read -r -d '' file; do
  rel=${file#"$ROOT"/}
  count=$((count + 1))
  if is_exempt "$rel"; then
    continue
  fi
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$LINE_LIMIT" ]; then
    echo "line-limit: 超限 $rel: ${lines} 行 > ${LINE_LIMIT} 行" >&2
    status=1
  fi
done < <(find "$CRATES_DIR" -type f -name '*.rs' -print0)

if [ "$status" -ne 0 ]; then
  echo "line-limit: FAIL（超限清单见上，红线 $LINE_LIMIT 行）" >&2
  exit 1
fi
if [ "$count" -eq 0 ]; then
  echo "line-limit: FAIL（$CRATES_DIR 下未找到任何 .rs 文件，扫描逻辑或目录异常）" >&2
  exit 1
fi
echo "line-limit: PASS（$count 个 .rs 文件全部 <= $LINE_LIMIT 行）"
