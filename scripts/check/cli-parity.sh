#!/usr/bin/env bash
# CLI 对等守卫（CL4 核心交付，防 GUI/CLI 命令面漂移）：
#   1. 机械提取 apps/gui/src-tauri/src/lib.rs generate_handler![...] 命令全集
#      （兼容 commands::xxx 与 <模块>::xxx 两种形态，取路径末段）；
#   2. 对照 scripts/check/cli-parity.tsv 映射表（机器可读 TSV）；
#   3. mapped 行用 p2pctl --help 实测存在性验证（递归解析 Commands: 段到叶子，
#      覆盖 chat friends list 三层路径），禁止只对表不验实际命令；
#   4. 缺映射 / 映射命令不存在 / 豁免无理由 / 表内陈旧行 → 输出清单并以非 0 退出。
# 成功时末行输出 CLI-PARITY-OK。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TSV="$ROOT/scripts/check/cli-parity.tsv"
GUI_LIB="$ROOT/apps/gui/src-tauri/src/lib.rs"
CTL="$ROOT/apps/cli/target/debug/p2pctl"

fail() { echo "cli-parity: FAIL $1" >&2; exit 1; }
[ -f "$GUI_LIB" ] || fail "找不到 $GUI_LIB"
[ -f "$TSV" ] || fail "找不到映射表 $TSV"

# --- 1. GUI 命令全集提取 ---
gui_cmds="$(awk '/generate_handler!\[/{flag=1} flag{print} flag && /\]/{exit}' "$GUI_LIB" \
  | grep -oE '[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*' \
  | awk -F'::' '{print $NF}' \
  | grep -v '^generate_handler$' \
  | sort -u)"
[ -n "$gui_cmds" ] || fail "未能从 generate_handler! 提取到任何命令"

gui_has() { printf '%s\n' "$gui_cmds" | grep -Fxq "$1"; }

# --- 2. p2pctl 就位（缺则构建，构建失败即红） ---
export PATH="$HOME/.cargo/bin:$PATH"
if [ ! -x "$CTL" ]; then
    echo "cli-parity: p2pctl 不存在，先构建 apps/cli…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml -q) \
        || fail "p2pctl 构建失败"
fi

# --- 3. 实测收集 CLI 命令面（递归 --help 的 Commands: 段到叶子） ---
cli_cmds_file="$(mktemp)"
trap 'rm -f "$cli_cmds_file"' EXIT
collect() {
    local path="$*" out subs sub
    out="$("$CTL" $path --help 2>&1)" || fail "执行 '$path --help' 失败"
    subs="$(printf '%s\n' "$out" \
        | awk '/^Commands:/{f=1;next} f&&/^[[:space:]]*$/{exit} f{print}' \
        | awk 'NF{print $1}' | grep -v '^help$' || true)"
    if [ -z "$subs" ]; then
        printf '%s\n' "$path" >> "$cli_cmds_file"
        return
    fi
    for sub in $subs; do
        collect $path $sub
    done
}
collect ""

cli_has() { grep -Fxq "$1" "$cli_cmds_file"; }

# --- 4. 对照映射表 ---
missing_mapping=""
stale_rows=""
bad_cmd=""
bad_exempt=""
mapped_count=0
exempt_count=0
while IFS= read -r tsv_line; do
    case "$tsv_line" in ''|'#'*) continue ;; esac
    # cut 按列取值：read + IFS 会把连续 TAB 折叠成单分隔符，空字段（如豁免行的
    # invocation）会被吞掉，导致理由列错位——这里必须用 cut 保空字段。
    gui="$(printf '%s\n' "$tsv_line" | cut -f1)"
    kind="$(printf '%s\n' "$tsv_line" | cut -f2)"
    invocation="$(printf '%s\n' "$tsv_line" | cut -f3)"
    reason="$(printf '%s\n' "$tsv_line" | cut -f4-)"
    if ! gui_has "$gui"; then
        stale_rows="$stale_rows  ${gui}（映射表有行，GUI 已无此命令）\n"
        continue
    fi
    case "$kind" in
        mapped)
            if [ -z "$invocation" ] || ! cli_has "$invocation"; then
                bad_cmd="$bad_cmd  $gui → [无 invocation]（p2pctl 实测无此命令）\n"
            else
                mapped_count=$((mapped_count + 1))
            fi
            ;;
        exempt)
            trimmed="$(printf '%s' "$reason" | tr -d ' ')"
            if [ -z "$trimmed" ]; then
                bad_exempt="$bad_exempt  ${gui}（豁免缺理由）\n"
            else
                exempt_count=$((exempt_count + 1))
            fi
            ;;
        *)
            bad_cmd="$bad_cmd  $gui → kind 非法（只允许 mapped|exempt）\n"
            ;;
    esac
done < "$TSV"

tsv_gui_column="$(grep -v '^#' "$TSV" | cut -f1)"
while IFS= read -r gui; do
    if ! printf '%s\n' "$tsv_gui_column" | grep -Fxq "$gui"; then
        missing_mapping="$missing_mapping  ${gui}（GUI 有命令，映射表无行）\n"
    fi
done <<< "$gui_cmds"

# --- 5. 汇总裁决 ---
rc=0
report() {
    local label="$1" list="$2"
    if [ -n "$list" ]; then
        echo "cli-parity: $label" >&2
        printf '%b' "$list" >&2
        rc=1
    fi
}
report "缺映射清单：" "$missing_mapping"
report "映射命令不存在清单：" "$bad_cmd"
report "豁免缺理由清单：" "$bad_exempt"
report "陈旧映射行清单：" "$stale_rows"
if [ "$rc" -ne 0 ]; then
    echo "cli-parity: FAIL" >&2
    exit 1
fi
leaf_count="$(grep -c . "$cli_cmds_file")"
gui_count="$(printf '%s\n' "$gui_cmds" | wc -l | tr -d ' ')"
echo "cli-parity: GUI 命令 $gui_count 个，映射 ${mapped_count}，豁免 ${exempt_count}；p2pctl 实测叶子命令 $leaf_count 个"
echo "CLI-PARITY-OK"
