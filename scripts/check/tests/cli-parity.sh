#!/usr/bin/env bash
# cli-parity 守卫自测（防守卫实现退化为假绿）：
#   正场景：夹具 GUI 命令与映射表、假 p2pctl 全部对得上 → 末行 CLI-PARITY-OK；
#   反场景：缺映射 / 映射命令不存在 / 豁免缺理由 / 陈旧行 四类失败都要红且非 0。
# 夹具树放独立临时目录，守卫按自身位置解析 ROOT，无需真实仓库与真 p2pctl。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
SRC="$ROOT/scripts/check"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/cli-parity-selftest.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/scripts/check" "$TMP/apps/gui/src-tauri/src" "$TMP/apps/cli/target/debug"
cp "$SRC/cli-parity.sh" "$TMP/scripts/check/"

# 假 p2pctl：顶层 node/metrics 两域，node 下仅 status 叶子（metrics 不设子命令域外叶子）
cat > "$TMP/apps/cli/target/debug/p2pctl" <<'FAKECTL'
#!/usr/bin/env bash
# 守卫固定带 --help 调用，先剥掉再按子命令路径匹配
args=()
for a in "$@"; do
  [ "$a" = "--help" ] || args+=("$a")
done
key="${args[*]}"
case "$key" in
  "") echo "Commands:"; echo "  node  n"; echo "  metrics  m" ;;
  node) echo "Commands:"; echo "  status  s" ;;
  *) echo "usage: fake" ;;
esac
FAKECTL
chmod +x "$TMP/apps/cli/target/debug/p2pctl"

lib_fixture() {  # $1 = 额外 GUI 命令行（可空）
  cat > "$TMP/apps/gui/src-tauri/src/lib.rs" <<LIBEOF
.invoke_handler(tauri::generate_handler![
    commands::node_start,
    commands::node_status,
${1}
])
LIBEOF
}

# --- 正场景：一一对应，含一条带理由豁免 ---
lib_fixture "    frontend_log::frontend_log_append,"
printf '%b\n' \
  "node_start\tmapped\tnode status\t" \
  "node_status\tmapped\tnode status\t" \
  "frontend_log_append\texempt\t\tGUI 前端专属行为，无采集源，理由充分" \
  > "$TMP/scripts/check/cli-parity.tsv"
out="$(bash "$TMP/scripts/check/cli-parity.sh" 2>&1)"
printf '%s\n' "$out" | tail -1 | grep -q "^CLI-PARITY-OK$" \
  || { echo "自测 FAIL：正场景应输出 CLI-PARITY-OK，实得：$out" >&2; exit 1; }

# --- 反场景：四类失败必须全部命中且非 0 ---
lib_fixture "    frontend_log::frontend_log_append,
    commands::identity_reset,"
printf '%b\n' \
  "node_start\tmapped\tnode status\t" \
  "node_status\tmapped\tnode missing\t" \
  "frontend_log_append\texempt\t\t" \
  "ghost_cmd\tmapped\tnode status\t" \
  > "$TMP/scripts/check/cli-parity.tsv"
rc=0
out="$(bash "$TMP/scripts/check/cli-parity.sh" 2>&1)" || rc=$?
[ "$rc" -ne 0 ] || { echo "自测 FAIL：反场景应非 0 退出" >&2; exit 1; }
for expect in "缺映射清单" "identity_reset" "映射命令不存在清单" "豁免缺理由清单" "陈旧映射行清单" "ghost_cmd"; do
    printf '%s\n' "$out" | grep -q "$expect" \
        || { echo "自测 FAIL：反场景缺 [$expect]，实得：$out" >&2; exit 1; }
done
echo "cli-parity-selftest: PASS"
