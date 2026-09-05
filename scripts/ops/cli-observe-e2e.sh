#!/usr/bin/env bash
# PR2 观测对等 E2E：node start --json 重定向 / node log tail / peer list /
# discovery list / relay status / log --data-dir 别名 六语义端到端验证。
# A/B 双节点回环互通（gui-config.json 预置 loopback 端点，不触公网）；
# 全程 --data-dir 临时目录隔离，trap 清理目录与进程（造数不过夜）。
# 重复执行安全：每次新建独立临时目录，退出时清理。末行输出 PR2-OBSERVE-OK。
set -eu
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="$ROOT/apps/cli/target/debug/p2pctl"
if [ ! -x "$CTL" ]; then
    echo "p2pctl 不存在，先构建…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/pr2-observe-e2e.XXXXXX")"
DA="$TMP/a"
DB="$TMP/b"
mkdir -p "$DA" "$DB"
PIDS=()

cleanup() {
    local pid
    "$CTL" node stop --data-dir "$DA" >/dev/null 2>&1 || true
    "$CTL" node stop --data-dir "$DB" >/dev/null 2>&1 || true
    for pid in "${PIDS[@]:-}"; do
        [ -n "$pid" ] && kill -9 "$pid" 2>/dev/null || true
    done
    rm -rf "$TMP"
}
trap cleanup EXIT

fail() { echo "E2E FAIL: $1" >&2; exit 1; }

# JSON 严格可解析断言（F9 回归核心：重定向文件非空且可解析）。
json_ok() {
    [ -s "$1" ] || fail "$2：文件为空"
    python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$1" \
        || fail "$2：JSON 不可解析"
}
json_field() {
    python3 -c "import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])" "$1" "$2"
}

# 回环封闭配置：显式 loopback 端点占位，避免出厂默认把流量打到公网
# （空列表会被装配层回落出厂默认，必须给非空值）。端口 0 = 随机。
write_hermetic_config() {
    cat > "$1/gui-config.json" <<JSON
{
  "quicPort": 0,
  "tcpPort": 0,
  "enableMdns": false,
  "bootstrap": ["127.0.0.1/u39400"],
  "relayAddrs": ["127.0.0.1/u39403"],
  "observationAddrs": ["127.0.0.1:39402"]
}
JSON
}

echo "== 1. START-REDIRECT：首次启动 --json 重定向文件非空且可解析 =="
write_hermetic_config "$DA"
write_hermetic_config "$DB"
"$CTL" node start --json --data-dir "$DA" > "$TMP/a-start.json" 2> "$TMP/a-start.err" \
    || fail "A 首次启动非零退出：$(cat "$TMP/a-start.err")"
json_ok "$TMP/a-start.json" "START-REDIRECT 首启"
A_PID="$(json_field "$TMP/a-start.json" pid)"
[ -n "$A_PID" ] || fail "START-REDIRECT 首启缺 pid 字段"
PIDS+=("$A_PID")

echo "== 2. START-REDIRECT：alreadyRunning 路径同样非空且可解析 =="
"$CTL" node start --json --data-dir "$DA" > "$TMP/a-again.json" 2>&1 \
    || fail "A 重复启动非零退出"
json_ok "$TMP/a-again.json" "START-REDIRECT alreadyRunning"
grep -q '"alreadyRunning": true' "$TMP/a-again.json" \
    || fail "重复启动未标 alreadyRunning=true：$(cat "$TMP/a-again.json")"

echo "== 3. 启动 B，取对端身份 =="
"$CTL" node start --json --data-dir "$DB" > "$TMP/b-start.json" 2>&1 \
    || fail "B 启动失败"
json_ok "$TMP/b-start.json" "B start"
B_PID="$(json_field "$TMP/b-start.json" pid)"
PIDS+=("$B_PID")
B_PEER="$(json_field "$TMP/b-start.json" peerId)"
B_ADDR="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['listenAddrs'][0])" "$TMP/b-start.json")"
[ -n "$B_PEER" ] && [ -n "$B_ADDR" ] || fail "取 B peerId/addr 失败"
echo "B peer=$B_PEER addr=$B_ADDR"

echo "== 4. A dial B 建立连接（PEER-LIST 前置）=="
"$CTL" peer dial "$B_PEER@$B_ADDR" --data-dir "$DA" > "$TMP/dial.txt" 2>&1 \
    || fail "A dial B 失败：$(cat "$TMP/dial.txt")"

echo "== 5. PEER-LIST：地址簿 + 在线态 =="
PEER_OUT="$("$CTL" peer list --data-dir "$DA")"
echo "$PEER_OUT" | grep -q "peer=$B_PEER connected=true" \
    || fail "PEER-LIST 缺 B 在线条目: $PEER_OUT"
"$CTL" peer list --json --data-dir "$DA" > "$TMP/peers.json"
json_ok "$TMP/peers.json" "PEER-LIST json"
python3 -c "import json,sys; d=json.load(open(sys.argv[1])); assert d['total'] >= 1 and d['connected'] >= 1" "$TMP/peers.json" \
    || fail "PEER-LIST json 计数不对"

echo "== 6. DISCOVERY-LIST：地址缓存（GUI 发现页口径）=="
DISC_OUT="$("$CTL" discovery list --data-dir "$DA")"
echo "$DISC_OUT" | grep -q "neighbor=$B_PEER" \
    || fail "DISCOVERY-LIST 缺 B 条目: $DISC_OUT"
echo "$DISC_OUT" | grep -q "^total=" || fail "DISCOVERY-LIST 缺来源计数汇总"
"$CTL" discovery list --json --data-dir "$DA" > "$TMP/disc.json"
json_ok "$TMP/disc.json" "DISCOVERY-LIST json"

echo "== 7. RELAY-STATUS：会话/水位只读快照 =="
RELAY_OUT="$("$CTL" relay status --data-dir "$DA")"
echo "$RELAY_OUT" | grep -q "^relaySessionsActive=" \
    || fail "RELAY-STATUS 缺会话水位行: $RELAY_OUT"
"$CTL" relay status --json --data-dir "$DA" > "$TMP/relay.json"
json_ok "$TMP/relay.json" "RELAY-STATUS json"
grep -q '"relaySessionsActive"' "$TMP/relay.json" || fail "RELAY-STATUS json 缺字段"

echo "== 8. NODE-LOG-TAIL：daemon.log 尾读语义 =="
LOG_OUT="$("$CTL" node log tail --lines 5 --data-dir "$DA")"
echo "$LOG_OUT" | grep -q "p2pctl-daemon: running pid=" \
    || fail "NODE-LOG-TAIL 缺守护进程启动行: $LOG_OUT"
LINE_COUNT="$("$CTL" node log tail --lines 1 --data-dir "$DA" | wc -l | tr -d ' ')"
[ "$LINE_COUNT" = "1" ] || fail "--lines 1 未按行钳制（实得 $LINE_COUNT 行）"
"$CTL" node log tail --json --lines 3 --data-dir "$DA" > "$TMP/log.json"
json_ok "$TMP/log.json" "NODE-LOG-TAIL json"
grep -q '"path"' "$TMP/log.json" || fail "NODE-LOG-TAIL json 缺 path"

echo "== 9. LOG-DATADIR：log 域兼容 --data-dir 别名 =="
ALIAS_PATH="$("$CTL" log path --data-dir "$DA")"
[ "$ALIAS_PATH" = "$DA/frontend.log" ] || fail "--data-dir 别名路径不对: $ALIAS_PATH"
NAMED_PATH="$("$CTL" log path --log-dir "$DA")"
[ "$NAMED_PATH" = "$DA/frontend.log" ] || fail "--log-dir 原名被破坏: $NAMED_PATH"

echo "== 10. PING-MICROS：亚毫秒 RTT 附 rttMicros（F13）=="
"$CTL" peer ping "$B_PEER" --json --data-dir "$DA" > "$TMP/ping.json" 2>&1 \
    || fail "A ping B 失败：$(cat "$TMP/ping.json")"
grep -q '"rttMicros"' "$TMP/ping.json" || fail "ping json 缺 rttMicros: $(cat "$TMP/ping.json")"

echo "== 11. 收尾：双双 stop，status 断言未运行 =="
"$CTL" node stop --data-dir "$DA" >/dev/null || fail "A stop 失败"
"$CTL" node stop --data-dir "$DB" >/dev/null || fail "B stop 失败"
PIDS=()
"$CTL" node status --data-dir "$DA" | grep -q "节点未运行" || fail "A stop 后仍报运行中"
"$CTL" node status --data-dir "$DB" | grep -q "节点未运行" || fail "B stop 后仍报运行中"

echo "PR2-OBSERVE-OK"