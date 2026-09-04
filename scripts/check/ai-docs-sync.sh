#!/usr/bin/env bash
# AI 文档防漂移守卫（N1 核心交付）：docs/ops/p2pctl-ai-guide.md 必须与 p2pctl 实测命令面同步。
#   1. 实测收集 p2pctl 叶子命令全集（--help 递归解析 Commands: 段到叶子，同 cli-parity.sh 手法）；
#   2. 正向：逐条断言文档命令目录区间（AI-DOCS-SYNC 标记内）含该命令条目（### p2pctl <path>）；
#   3. 反向：文档条目在实现无此命令 → 红（防文档陈旧/超前）；
#   4. 参数机械比对（覆盖全部叶子，超出"至少 3 个抽查"要求）：
#      --help Options 段长参数名与文档条目参数表双向逐字一致，Arguments 位置参数占位符同检；
#   5. 示例抽验：文档示例命令实测可跑，抽查 6 条覆盖退出码 0/1/2 全语义。
# 成功时末行输出 AI-DOCS-OK。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DOC="$ROOT/docs/ops/p2pctl-ai-guide.md"
CTL="$ROOT/apps/cli/target/debug/p2pctl"

fail() { echo "ai-docs-sync: FAIL $1" >&2; exit 1; }
[ -f "$DOC" ] || fail "找不到 $DOC"

export PATH="$HOME/.cargo/bin:$PATH"
if [ ! -x "$CTL" ]; then
    echo "ai-docs-sync: p2pctl 不存在，先构建 apps/cli…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml -q) || fail "p2pctl 构建失败"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- 1. 递归收集叶子命令与各自 help 明细 ---
LEAVES="$TMP/leaves.txt"; : > "$LEAVES"
collect() {
    local path="$*" out subs sub hf
    out="$("$CTL" $path --help 2>&1)" || fail "执行 '$path --help' 失败"
    subs="$(printf '%s\n' "$out" \
        | awk '/^Commands:/{f=1;next} f&&/^[[:space:]]*$/{exit} f{print}' \
        | awk 'NF{print $1}' | grep -v '^help$' || true)"
    if [ -z "$subs" ]; then
        printf '%s\n' "$path" >> "$LEAVES"
        hf="$TMP/help_$(printf '%s' "$path" | tr ' ' '_')"
        printf '%s\n' "$out" > "$hf"
        return
    fi
    for sub in $subs; do collect $path $sub; done
}
collect ""
leaf_count="$(grep -c . "$LEAVES")"
[ "$leaf_count" -gt 0 ] || fail "未收集到任何叶子命令"

# --- 2. 解析文档命令目录区间 ---
doc_region="$(awk '/AI-DOCS-SYNC:BEGIN/{f=1;next} /AI-DOCS-SYNC:END/{f=0} f' "$DOC")"
[ -n "$doc_region" ] || fail "文档缺 AI-DOCS-SYNC 校验区间标记"
doc_paths="$(printf '%s\n' "$doc_region" | grep -E '^### p2pctl ' | sed 's/^### p2pctl //' | sort -u)"
doc_count="$(printf '%s\n' "$doc_paths" | grep -c . || true)"

entry_block() {
    printf '%s\n' "$doc_region" | awk -v h="### p2pctl $1" '
        $0==h{f=1;next}
        f&&/^### p2pctl /{exit}
        f{print}'
}

# --- 3. 正向 + 参数机械比对（全量叶子，双向） ---
missing_entry=""
param_detail=""
checked_params=0
while IFS= read -r leaf; do
    hf="$TMP/help_$(printf '%s' "$leaf" | tr ' ' '_')"
    blk="$(entry_block "$leaf")"
    if [ -z "$blk" ]; then
        missing_entry="$missing_entry  p2pctl $leaf（实现有，文档无条目）\n"
        continue
    fi
    # Options 段每行取首个 --长参数（flag 必在行首位）；--help 为通用参数不进比对
    help_opts="$(awk '/^Options:/{f=1;next} f{ if (match($0, /--[a-z][a-z0-9-]*/)) print substr($0, RSTART, RLENGTH) }' "$hf" | sort -u | grep -v -e '^--help$' || true)"
    help_args="$(awk '/^Arguments:/{f=1;next} f&&/^[[:space:]]*$/{exit} f' "$hf" \
        | grep -oE -e '<[A-Z][A-Z_]*>|\[[A-Z][A-Z_]*\]' | sort -u || true)"
    doc_opts="$(printf '%s\n' "$blk" | grep -oE -e '--[a-z][a-z0-9-]*' | sort -u || true)"
    doc_args="$(printf '%s\n' "$blk" | grep -oE -e '<[A-Z][A-Z_]*>|\[[A-Z][A-Z_]*\]' | sort -u || true)"
    while IFS= read -r o; do
        [ -n "$o" ] || continue
        if ! printf '%s\n' "$doc_opts" | grep -Fxq -e "$o"; then
            param_detail="$param_detail  p2pctl $leaf：文档条目缺参数 $o\n"
        fi
        checked_params=$((checked_params + 1))
    done <<< "$help_opts"
    while IFS= read -r o; do
        [ -n "$o" ] || continue
        if ! printf '%s\n' "$help_opts" | grep -Fxq -e "$o"; then
            param_detail="$param_detail  p2pctl $leaf：文档参数 $o 实现不存在\n"
        fi
    done <<< "$doc_opts"
    while IFS= read -r a; do
        [ -n "$a" ] || continue
        if ! printf '%s\n' "$blk" | grep -Fq -e "$a"; then
            param_detail="$param_detail  p2pctl $leaf：文档条目缺位置参数 $a\n"
        fi
    done <<< "$help_args"
done < "$LEAVES"

# --- 4. 反向：文档有而实现无 ---
stale_entry=""
while IFS= read -r p; do
    [ -n "$p" ] || continue
    if ! grep -Fxq -e "$p" "$LEAVES"; then
        stale_entry="$stale_entry  p2pctl $p（文档有，实现无此命令）\n"
    fi
done <<< "$doc_paths"

# --- 5. 示例抽验：文档示例命令实测可跑（覆盖退出码 0/1/2） ---
rc=0
run_sample() {
    local doc_grep="$1" args="$2" want="$3" got
    if ! grep -Fq -e "$doc_grep" "$DOC"; then
        echo "ai-docs-sync: FAIL 文档缺示例命令 $doc_grep" >&2
        rc=1
        return
    fi
    set +e
    "$CTL" $args > "$TMP/sample.out" 2> "$TMP/sample.err"
    got=$?
    set -e
    if [ "$got" -ne "$want" ]; then
        echo "ai-docs-sync: FAIL 示例 '$args' 实测退出码 $got，期望 $want（stderr: $(head -1 "$TMP/sample.err")）" >&2
        rc=1
    fi
}
SAMPLE_DIR="$TMP/sample-data"
mkdir -p "$SAMPLE_DIR"
run_sample "p2pctl node status"       "node status --json --data-dir $SAMPLE_DIR"        0
run_sample "p2pctl chat friends list" "chat friends list --json --data-dir $SAMPLE_DIR"  0
run_sample "p2pctl config get"        "config get --json --data-dir $SAMPLE_DIR"         0
run_sample "p2pctl metrics get"       "metrics get --json --data-dir $SAMPLE_DIR"        0
run_sample "p2pctl identity reset"    "identity reset --data-dir $SAMPLE_DIR"            1
run_sample "p2pctl chat send"         "chat send --text hi"                               2
[ "$rc" -eq 0 ] || exit 1

# --- 6. 汇总裁决 ---
rc=0
report() {
    local label="$1" list="$2"
    if [ -n "$list" ]; then
        echo "ai-docs-sync: $label" >&2
        printf '%b' "$list" >&2
        rc=1
    fi
}
report "文档缺条目清单：" "$missing_entry"
report "文档多出/陈旧条目清单：" "$stale_entry"
report "参数不一致清单：" "$param_detail"
if [ "$rc" -ne 0 ]; then
    echo "ai-docs-sync: FAIL" >&2
    exit 1
fi
echo "ai-docs-sync: p2pctl 实测叶子 $leaf_count 个，文档条目 $doc_count 个，参数名比对 $checked_params 项，示例抽验 6 条（退出码 0/1/2）"
echo "AI-DOCS-OK"
