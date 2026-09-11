#!/usr/bin/env bash
# 协议注册表机械门禁：代码字面量 <-> registry.toml <-> wire-protocol.md 四向核对
# 数据源：docs/protocol/registry.toml（唯一真值，字段语义见 docs/protocol/spec-charter.md §6）
# 四向核对：
#   a) 扫描 crates/ apps/ 全部 .rs 中协议 ID 形态字符串字面量
#      （"/段(/段)*/纯数字版本"，段字符集 [a-z0-9_-]，整体引号包裹）
#   b) 按 registry test_namespaces（单行数组）过滤测试命名空间后，
#      每个公开字面量必须已登记于 registry，违规逐条列出并 exit 1
#   c) registry 每个 id 必须出现在代码字面量中，或其 impl="planned"
#   d) registry 每个 id 必须在 docs/design/wire-protocol.md 有登记（grep 固定串）
# 豁免规则（豁免清单只增不减，新增需负责人裁决）：
#   - test_namespaces 前缀命中的字面量视为测试专用，不构成公开协议面
#   - 测试代码不计公开面（与 panic-hygiene.sh 同口径）：#[cfg(test)] 模块体、
#     tests/ examples/ benches/ 目录、tests.rs 与 *_tests.rs 约定文件
# 测试钩子：CHECK_ROOT 覆盖仓库根（夹具驱动）；--self-test 在 mktemp 沙箱自证红绿
# 局限：文本扫描；TOML 数组须单行且行内无注释；块注释与跨行字符串不解析；
#   版本段须纯数字（1.0.0 多段版本不匹配）；#[cfg(test)] 单条目形态不展开跳过
set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SELF="$SCRIPT_DIR/$(basename "${BASH_SOURCE[0]}")"

# 从 registry 按行提取 test_namespaces 单行数组，输出空格分隔的前缀串
parse_namespaces() {
  sed -n 's/^test_namespaces[[:space:]]*=[[:space:]]*//p' "$1" \
    | sed -E 's/^\[(.*)\][[:space:]]*$/\1/; s/"//g; s/,/ /g'
}

# 按行提取 [[protocols]] 块的 id 与 impl，输出 TSV（id<TAB>impl）
parse_protocol_ids() {
  awk '
    function qstr(s) { if (match(s, /"[^"]*"/)) return substr(s, RSTART + 1, RLENGTH - 2); return "" }
    /^[[:space:]]*\[\[protocols\]\]/ { if (id != "") print id "\t" impl; id = ""; impl = ""; next }
    /^[[:space:]]*id[[:space:]]*=/ { if (id == "") id = qstr($0); next }
    /^[[:space:]]*impl[[:space:]]*=/ { impl = qstr($0); next }
    END { if (id != "") print id "\t" impl }
  ' "$1"
}

# 输出 file:line: id（跳过行注释与 #[cfg(test)] 模块体，状态机同 panic-hygiene.sh）
scan_file() {
  awk -v rel="$2" '
    /^[[:space:]]*\/\// { next }
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
    {
      line = $0
      while (match(line, /"[^"]*"/)) {
        s = substr(line, RSTART + 1, RLENGTH - 2)
        if (s ~ /^\/[a-z0-9_-]+(\/[a-z0-9_-]+)*\/[0-9]+$/) print rel ":" FNR ": " s
        line = substr(line, RSTART + RLENGTH)
      }
    }
  ' "$1"
}

# 字面量是否命中 test_namespaces 前缀（NS_LIST 为空格分隔前缀串）
is_test_ns() {
  local ns
  for ns in $NS_LIST; do
    case "$1" in "$ns"*) return 0 ;; esac
  done
  return 1
}

main() {
  local root="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
  local registry="$root/docs/protocol/registry.toml"
  local wire_doc="$root/docs/design/wire-protocol.md"
  local dir
  for dir in "$registry" "$wire_doc"; do
    if [ ! -f "$dir" ]; then
      echo "protocol-registry: FAIL 数据源缺失: $dir" >&2
      exit 1
    fi
  done
  if [ ! -d "$root/crates" ] && [ ! -d "$root/apps" ]; then
    echo "protocol-registry: FAIL 找不到 crates/ 与 apps/ 目录" >&2
    exit 1
  fi

  NS_LIST="$(parse_namespaces "$registry")"
  if [ -z "$NS_LIST" ]; then
    echo "protocol-registry: FAIL registry 缺 test_namespaces（数据源不合规）" >&2
    exit 1
  fi

  work=$(mktemp -d) || exit 1
  trap 'rm -rf "$work"' EXIT
  parse_protocol_ids "$registry" > "$work/ids.tsv"
  if [ ! -s "$work/ids.tsv" ]; then
    echo "protocol-registry: FAIL registry 未解析到任何 [[protocols]] 块" >&2
    exit 1
  fi

  # 方向 a：全量扫描出定位行与唯一 ID 集
  local status=0 scanned=0 rel file
  while IFS= read -r -d '' file; do
    rel=${file#"$root"/}
    scanned=$((scanned + 1))
    scan_file "$file" "$rel"
  done < <(find "$root/crates" "$root/apps" -type f -name '*.rs' \
    ! -path '*/tests/*' ! -path '*/examples/*' ! -path '*/benches/*' \
    ! -name 'tests.rs' ! -name '*_tests.rs' -print0 2>/dev/null) > "$work/hits"
  awk -F': ' '{print $NF}' "$work/hits" | sort -u > "$work/found"
  if [ "$scanned" -eq 0 ]; then
    echo "protocol-registry: FAIL 未扫描到任何 .rs 文件（目录或排除规则异常）" >&2
    exit 1
  fi

  # 方向 b：公开字面量必须已登记
  local id bad_b=0
  while IFS= read -r id; do
    is_test_ns "$id" && continue
    if ! awk -F'\t' -v x="$id" '$1 == x { found = 1 } END { exit !found }' "$work/ids.tsv"; then
      echo "protocol-registry: FAIL 未登记的公开协议 ID 字面量（应登记进 registry.toml 或改用测试命名空间）:" >&2
      awk -F': ' -v x="$id" '$NF == x' "$work/hits" >&2
      bad_b=1
    fi
  done < "$work/found"

  # 方向 c：registry id 必须有代码字面量，或 impl="planned"
  local impl bad_c=0
  while IFS=$'\t' read -r id impl; do
    [ "$impl" = "planned" ] && continue
    if ! grep -Fxq -- "$id" "$work/found"; then
      echo "protocol-registry: FAIL registry 已实现 id 缺代码字面量: $id" >&2
      bad_c=1
    fi
  done < "$work/ids.tsv"

  # 方向 d：registry id 必须在 wire-protocol.md 有登记
  local bad_d=0
  while IFS=$'\t' read -r id _; do
    if ! grep -Fq -- "$id" "$wire_doc"; then
      echo "protocol-registry: FAIL wire-protocol.md 缺登记: $id" >&2
      bad_d=1
    fi
  done < "$work/ids.tsv"

  if [ "$bad_b" -ne 0 ] || [ "$bad_c" -ne 0 ] || [ "$bad_d" -ne 0 ]; then
    echo "protocol-registry: FAIL 四向核对未过（b 未登记=${bad_b} c 缺实现=${bad_c} d 缺文档=${bad_d}）" >&2
    exit 1
  fi
  local found_public=0
  while IFS= read -r id; do
    is_test_ns "$id" || found_public=$((found_public + 1))
  done < "$work/found"
  echo "protocol-registry: PASS（扫描 $scanned 个文件，公开字面量 $found_public 个全部登记；registry $(wc -l < "$work/ids.tsv" | tr -d ' ') 个 id 全核对；wire-protocol 登记全量）"
}

self_test() {
  local pass=0 fail=0 base out rc name want_rc want_out
  base=$(mktemp -d) || return 1
  t() { # t <名称> <期望退出码> <输出须含> <夹具目录>
    name="$1"; want_rc="$2"; want_out="$3"
    out="$(env CHECK_ROOT="$4" bash "$SELF" 2>&1)"; rc=$?
    if [ "$rc" -eq "$want_rc" ] && printf '%s' "$out" | grep -qF -- "$want_out"; then
      pass=$((pass + 1)); echo "  ok   $name"
    else
      fail=$((fail + 1))
      echo "  FAIL ${name}（rc=$rc 期望 ${want_rc}，输出应含 '$want_out'）" >&2
      printf '%s\n' "$out" | sed 's/^/    | /' >&2
    fi
  }
  make_fixture() { # make_fixture <dir> <带 echo 常量: 0|1>
    mkdir -p "$1/docs/protocol" "$1/docs/design" "$1/crates/demo/src" "$1/crates/demo/tests"
    printf '%s\n' 'schema = 1' 'test_namespaces = ["/itest/"]' '' \
      '[[protocols]]' 'id = "/demo/echo/1"' 'impl = "implemented"' '' \
      '[[protocols]]' 'id = "/demo/planned/1"' 'impl = "planned"' > "$1/docs/protocol/registry.toml"
    printf '%s\n' '# wire doc' '| /demo/echo/1 | fixture |' '| /demo/planned/1 | fixture |' \
      > "$1/docs/design/wire-protocol.md"
    if [ "$2" = "1" ]; then
      printf 'pub const ECHO: &str = "/demo/echo/1";\n' > "$1/crates/demo/src/lib.rs"
    else
      printf 'pub fn demo() {}\n' > "$1/crates/demo/src/lib.rs"
    fi
    printf 'pub fn it() {}\n' > "$1/crates/demo/tests/it.rs"
  }

  echo "== 绿路径 =="
  G="$base/green"; make_fixture "$G" 1
  t "干净夹具通过（implemented 有字面量 + planned 免实现）" 0 "protocol-registry: PASS" "$G"
  printf 'pub const NS: &str = "/itest/ghost/1";\n' >> "$G/crates/demo/src/lib.rs"
  t "测试命名空间字面量豁免" 0 "protocol-registry: PASS" "$G"
  printf '%s\n' '#[cfg(test)]' 'mod tests {' '    #[test]' '    fn inner() {' '        let _ = "/demo/ghost/1";' '    }' '}' >> "$G/crates/demo/src/lib.rs"
  t "cfg(test) 模块内未登记 ID 不计" 0 "protocol-registry: PASS" "$G"
  printf 'pub const T: &str = "/demo/ghost/1";\n' > "$G/crates/demo/tests/it.rs"
  t "tests/ 目录未登记 ID 不计" 0 "protocol-registry: PASS" "$G"

  echo "== 红路径 =="
  R="$base/red"; make_fixture "$R" 1
  printf 'pub const GHOST: &str = "/demo/ghost/1";\n' >> "$R/crates/demo/src/lib.rs"
  t "未登记公开 ID 判红" 1 "protocol-registry: FAIL" "$R"
  t "违规定位到植入行" 1 "/demo/ghost/1" "$R"
  C="$base/missing-impl"; make_fixture "$C" 0
  t "implemented id 缺代码字面量判红" 1 "缺代码字面量: /demo/echo/1" "$C"
  D="$base/missing-doc"; make_fixture "$D" 1
  grep -v '/demo/echo/1' "$D/docs/design/wire-protocol.md" > "$D/w.tmp" && mv "$D/w.tmp" "$D/docs/design/wire-protocol.md"
  t "wire-protocol.md 缺登记判红" 1 "wire-protocol.md 缺登记: /demo/echo/1" "$D"

  echo "== 防假绿 =="
  t "registry 缺失拒绝" 1 "数据源缺失" "$base/nodata"
  N="$base/no-ns"; make_fixture "$N" 1
  grep -v '^test_namespaces' "$N/docs/protocol/registry.toml" > "$N/r.tmp" && mv "$N/r.tmp" "$N/docs/protocol/registry.toml"
  t "test_namespaces 缺失拒绝" 1 "数据源不合规" "$N"
  rm -rf "$base"
  if [ "$fail" -ne 0 ]; then
    echo "protocol-registry 自测：FAIL（$pass 通过 / $fail 失败）" >&2
    return 1
  fi
  echo "protocol-registry 自测：PASS（$pass 通过）"
}

case "${1:-}" in
  --self-test) self_test; exit $? ;;
  "") main ;;
  *) echo "protocol-registry: 用法: $0 [--self-test]" >&2; exit 2 ;;
esac
