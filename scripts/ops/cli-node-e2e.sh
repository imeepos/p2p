#!/usr/bin/env bash
# CL2 E2E：p2pctl node/config/peer 命令域端到端验证。
# 全程 --data-dir 临时目录隔离；起 A/B 双节点 → A dial/connect B → A ping B →
# 双双 stop → status 断言未运行；trap 清理全部临时目录与进程（造数不过夜）。
# 重复执行安全：每次新建独立临时目录，退出时清理。末行输出 CL2-E2E-OK。
set -eu
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="$ROOT/apps/cli/target/debug/p2pctl"
if [ ! -x "$CTL" ]; then
    echo "p2pctl 不存在，先构建…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/cl2-node-e2e.XXXXXX")"
DA="$TMP/a"
DB="$TMP/b"
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

# 从文本输出取 key=value 行值（grep -E "^key="），无 JSON 依赖。
keyof() { printf "%s\n" "$1" | grep -E "^$2=" | head -1 | cut -d= -f2-; }

echo "== 1. 启动 A/B 双节点 =="
A_OUT="$("$CTL" node start --data-dir "$DA")"
A_PID="$(keyof "$A_OUT" pid)"
[ -n "$A_PID" ] || fail "A start 输出缺 pid 行: $A_OUT"
PIDS+=("$A_PID")
B_OUT="$("$CTL" node start --data-dir "$DB")"
B_PID="$(keyof "$B_OUT" pid)"
[ -n "$B_PID" ] || fail "B start 输出缺 pid 行: $B_OUT"
PIDS+=("$B_PID")
echo "A pid=$A_PID / B pid=$B_PID"

echo "== 2. status 断言运行中（JSON 形态）=="
A_STATUS_JSON="$("$CTL" node status --data-dir "$DA" --json)"
echo "$A_STATUS_JSON" | grep -q '"running": true' || fail "A status 未报运行中: $A_STATUS_JSON"
echo "$A_STATUS_JSON" | grep -q '"peerId"' || fail "A status 缺 peerId"

echo "== 3. A dial B（显式地址）=="
B_STATUS_JSON="$("$CTL" node status --data-dir "$DB" --json)"
B_PEER="$(printf "%s" "$B_STATUS_JSON" | grep -E '"peerId"' | head -1 | sed -E 's/.*"peerId": "([^"]+)".*/\1/')"
B_ADDR="$(printf "%s" "$B_STATUS_JSON" | grep -E '"[^" ]+/u[0-9]+"' | head -1 | sed -E 's|.*"([^"]+/u[0-9]+)".*|\1|')"
[ -n "$B_PEER" ] || fail "取 B peerId 失败: $B_STATUS_JSON"
[ -n "$B_ADDR" ] || fail "取 B 监听地址失败: $B_STATUS_JSON"
B_ADDR="${B_ADDR//0.0.0.0/127.0.0.1}"
echo "dial target=$B_PEER@$B_ADDR"
DIAL_OUT="$("$CTL" peer dial "$B_PEER@$B_ADDR" --data-dir "$DA")"
echo "$DIAL_OUT" | grep -q "^已连接" || fail "dial 未成功: $DIAL_OUT"

echo "== 4. A peer ping B（真实 RTT）=="
PING_OUT="$("$CTL" peer ping "$B_PEER" --data-dir "$DA")"
echo "$PING_OUT"
echo "$PING_OUT" | grep -q "rtt_ms=[0-9]" || fail "ping 缺真实 RTT: $PING_OUT"

echo "== 5. disconnect 幂等 =="
"$CTL" peer disconnect "$B_PEER" --data-dir "$DA" >/dev/null || fail "disconnect 失败"
"$CTL" peer disconnect "$B_PEER" --data-dir "$DA" >/dev/null || fail "重复 disconnect 失败"

echo "== 6. 双双 stop =="
"$CTL" node stop --data-dir "$DA" >/dev/null || fail "A stop 失败"
"$CTL" node stop --data-dir "$DB" >/dev/null || fail "B stop 失败"
PIDS=()

echo "== 7. status 断言未运行 + 重复 stop 幂等 =="
A_AFTER="$("$CTL" node status --data-dir "$DA")"
echo "$A_AFTER" | grep -q "节点未运行" || fail "A stop 后仍报运行中: $A_AFTER"
B_AFTER="$("$CTL" node status --data-dir "$DB")"
echo "$B_AFTER" | grep -q "节点未运行" || fail "B stop 后仍报运行中: $B_AFTER"
"$CTL" node stop --data-dir "$DA" >/dev/null || fail "重复 stop A 非零退出"
"$CTL" node stop --data-dir "$DB" >/dev/null || fail "重复 stop B 非零退出"

echo "== 8. 空态 config/profile get（对齐 GUI 首跑）=="
"$CTL" config get --data-dir "$DA" | grep -q "^enableMdns=true" || fail "config get 空态缺默认值"
"$CTL" profile get --data-dir "$DA" | grep -q "^name=$" || fail "profile get 空态非默认"

echo "== 9. identity reset 必须显式 confirm =="
if "$CTL" identity reset --data-dir "$DA" >/dev/null 2>&1; then
    fail "缺 --confirm 的 identity reset 竟然成功"
fi

echo "CL2-E2E-OK"
