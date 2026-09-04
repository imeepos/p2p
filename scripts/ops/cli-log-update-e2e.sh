#!/usr/bin/env bash
# CL4 E2E：p2pctl log / metrics / update 命令域端到端验证。
# 全程临时目录隔离（--log-dir / --data-dir），不触真实外网（update 仅验白名单输出，
# 不做在线 check）；起真实验证 metrics 控制通道往返后 stop；trap 清理（造数不过夜）。
# 重复执行安全。末行输出 CL4-E2E-OK。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="$ROOT/apps/cli/target/debug/p2pctl"
if [ ! -x "$CTL" ]; then
    echo "p2pctl 不存在，先构建…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/cl4-e2e.XXXXXX")"
LOGDIR="$TMP/logs"
DATADIR="$TMP/data"
PIDS=()
cleanup() {
    local pid
    for pid in "${PIDS[@]:-}"; do
        [ -n "$pid" ] && kill -9 "$pid" 2>/dev/null || true
    done
    rm -rf "$TMP"
}
trap cleanup EXIT

fail() { echo "E2E FAIL: $1" >&2; exit 1; }

echo "== 1. log tail：文本与行数参数 =="
mkdir -p "$LOGDIR"
printf 'line-1\nline-2\nline-3\n' > "$LOGDIR/frontend.log"
OUT="$("$CTL" log tail --log-dir "$LOGDIR")"
[ "$OUT" = "$(printf 'line-1\nline-2\nline-3')" ] || fail "log tail 全量不符: $OUT"
OUT="$("$CTL" log tail --log-dir "$LOGDIR" --lines 2)"
[ "$OUT" = "$(printf 'line-2\nline-3')" ] || fail "log tail --lines 2 不符: $OUT"

echo "== 2. log tail：JSON 形态 =="
"$CTL" log tail --log-dir "$LOGDIR" --json | grep -q '"line-3"' || fail "log tail JSON 缺末行"

echo "== 3. log path =="
"$CTL" log path --log-dir "$LOGDIR" | grep -q "frontend.log$" || fail "log path 输出异常"

echo "== 4. log clear：删除 + 幂等 =="
"$CTL" log clear --log-dir "$LOGDIR" | grep -q "current=true" || fail "clear 未删当前代"
[ ! -f "$LOGDIR/frontend.log" ] || fail "clear 后 frontend.log 仍存在"
"$CTL" log clear --log-dir "$LOGDIR" --json | grep -q '"removedCurrent": false' \
    || fail "clear 不幂等"

echo "== 5. update open：白名单校验 + URL 输出（不开浏览器）=="
OUT="$("$CTL" update open --url https://github.com/imeepos/p2p/releases)"
[ "$OUT" = "https://github.com/imeepos/p2p/releases" ] || fail "update open 输出异常: $OUT"
if "$CTL" update open --url https://evil.com/x >/dev/null 2>&1; then
    fail "白名单外 URL 竟然成功"
fi

echo "== 6. metrics get：离线全零（同 GUI 未运行语义）=="
"$CTL" metrics get --data-dir "$DATADIR" | grep -q "^activeConnections=0$" \
    || fail "metrics 离线缺 activeConnections=0"

echo "== 7. metrics get：在线走 daemon.sock 控制通道 =="
START_OUT="$("$CTL" node start --data-dir "$DATADIR")"
PID="$(printf "%s\n" "$START_OUT" | grep -E "^pid=" | head -1 | cut -d= -f2-)"
[ -n "$PID" ] || fail "node start 缺 pid 行: $START_OUT"
PIDS+=("$PID")
MJSON="$("$CTL" metrics get --data-dir "$DATADIR" --json)"
echo "$MJSON" | grep -q '"dialDirectOk"' || fail "在线 metrics 缺 dialDirectOk: $MJSON"
"$CTL" node stop --data-dir "$DATADIR" >/dev/null || fail "node stop 失败"
PIDS=()

echo "CL4-E2E-OK"
