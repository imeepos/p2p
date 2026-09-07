#!/bin/bash
# 本地 ACP 全链冒烟（机械验收）：真 acp-agent + 真 acp-console + node WS 客户端
# 走完 握手→initialize→session/new→prompt 往返；admin 工作区 CRUD 往返。
# 验收口径：输出 ACP-LOCAL-SMOKE-OK 且退出码 0。
# 真 dsh 探针在 scripts/ops/acp-local-setup.sh 尾部单独做（不可用即 SKIP 不假绿）。
set -u
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d /tmp/acp-local-smoke.XXXXXX)"
AGENT_BIN="$REPO/apps/acp-agent/target/release/acp-agent"
STUB_BIN="$REPO/apps/acp-agent/target/release/acp-echo-stub"
CONSOLE_BIN="$REPO/apps/acp-console/target/release/acp-console"
AGENT_PORT=17811
ADMIN_PORT=17812
WS_PORT=17813
AGENT_PID=""
CONSOLE_PID=""
cleanup() {
  [ -n "$CONSOLE_PID" ] && kill "$CONSOLE_PID" 2>/dev/null
  [ -n "$AGENT_PID" ] && kill "$AGENT_PID" 2>/dev/null
  wait 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT
fail() { echo "ACP-LOCAL-SMOKE-FAIL: $*" >&2; exit 1; }
for bin in "$AGENT_BIN" "$STUB_BIN" "$CONSOLE_BIN"; do
  [ -x "$bin" ] || fail "缺少 $bin（先跑 scripts/ops/acp-local-setup.sh 构建）"
done
command -v node >/dev/null || fail "node 不可用"
PCTL="$REPO/apps/cli/target/release/p2pctl"
[ -x "$PCTL" ] || fail "缺少 $PCTL（先跑 scripts/ops/acp-local-setup.sh 构建）"

# 1) 先起操作者侧 console：拿到 console PeerId 写授权，再起 agent
mkdir -p "$WORK/console"
"$CONSOLE_BIN" --data-dir "$WORK/console" --no-mdns --ws-port "$WS_PORT" --status-port 0 \
  >"$WORK/console.out" 2>"$WORK/console.err" &
CONSOLE_PID=$!
ready_field() {
  # stdout 可能已追加 state/discovery 行：只取 ready 行解析
  grep -h '"kind":"ready"' "$1" 2>/dev/null | head -1 |
    python3 -c 'import json,sys; print(json.load(sys.stdin)[sys.argv[1]])' "$2" 2>/dev/null
}
CONSOLE_PEER=""
for _ in $(seq 1 50); do
  grep -q '"kind":"ready"' "$WORK/console.out" 2>/dev/null && CONSOLE_PEER="$(ready_field "$WORK/console.out" peer)"
  [ -n "$CONSOLE_PEER" ] && break
  sleep 0.2
done
[ -n "$CONSOLE_PEER" ] || fail "console 未就绪（$(tail -2 "$WORK/console.err" 2>/dev/null)）"
WS_URL="$(ready_field "$WORK/console.out" ws)"
WS_TOKEN="$(ready_field "$WORK/console.out" token)"
echo "[1] console ready peer=$CONSOLE_PEER ws=$WS_URL"

# 2) 写策略（scope=sandbox 顺带覆盖 jail 路径），再起 agent
"$PCTL" acp allow "$CONSOLE_PEER" --scope sandbox --data-dir "$WORK/agent" --note smoke >/dev/null \
  || fail "p2pctl acp allow 失败"
"$AGENT_BIN" --data-dir "$WORK/agent" --quic-port "$AGENT_PORT" --admin-port "$ADMIN_PORT" \
  --descriptor-disabled --command "$STUB_BIN" >"$WORK/agent.out" 2>"$WORK/agent.err" &
AGENT_PID=$!
AGENT_PEER=""
for _ in $(seq 1 50); do
  AGENT_PEER="$(grep -o 'running peer=[^ ]*' "$WORK/agent.err" 2>/dev/null | head -1 | cut -d= -f2)"
  [ -n "$AGENT_PEER" ] && break
  sleep 0.2
done
[ -n "$AGENT_PEER" ] || fail "agent 未就绪（$(tail -2 "$WORK/agent.err" 2>/dev/null)）"
echo "[2] agent ready peer=$AGENT_PEER quic=$AGENT_PORT"

# 2.5) 重启 console（同数据目录=同身份）并登记 agent 地址候选：--no-mdns 下
# console 无发现面，不登记候选 dial 直接失败（close 4500）。
kill "$CONSOLE_PID" 2>/dev/null
wait "$CONSOLE_PID" 2>/dev/null
"$CONSOLE_BIN" --data-dir "$WORK/console" --no-mdns --ws-port "$WS_PORT" --status-port 0 \
  --peer "$AGENT_PEER@127.0.0.1/u$AGENT_PORT" >"$WORK/console2.out" 2>"$WORK/console2.err" &
CONSOLE_PID=$!
for _ in $(seq 1 50); do
  grep -q '"kind":"ready"' "$WORK/console2.out" 2>/dev/null && break
  sleep 0.2
done
WS_URL="$(ready_field "$WORK/console2.out" ws)"
WS_TOKEN="$(ready_field "$WORK/console2.out" token)"
echo "[2.5] console restarted with manual candidate $AGENT_PEER@127.0.0.1:$AGENT_PORT"

# 3) WS 全链：initialize → session/new → prompt 往返（哑泵两段都是真件）
node "$REPO/scripts/ops/acp-local-ws-probe.mjs" "$WS_URL" "$WS_TOKEN" "$AGENT_PEER" 25 \
  >"$WORK/ws-probe.out" 2>"$WORK/ws-probe.err" || fail "WS 探针失败（$(cat "$WORK/ws-probe.err")）"
cat "$WORK/ws-probe.out"
grep -q "WS-PROBE-OK" "$WORK/ws-probe.out" || fail "WS 探针未达 OK"

# 4) admin 工作区 CRUD 往返（POST→GET 含行→DELETE→GET 不含行）
TOKEN="$(cat "$WORK/agent/acp-admin-token")"
ADMIN="http://127.0.0.1:$ADMIN_PORT"
code="$(curl -s -o "$WORK/ws-add.json" -w '%{http_code}' -X POST "$ADMIN/workspaces" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"id":"docs","name":"Docs","dir":"/tmp"}')"
[ "$code" = "200" ] || fail "POST /workspaces 期望 200 得 $code"
curl -s "$ADMIN/workspaces" -H "Authorization: Bearer $TOKEN" | grep -q '"docs"' || fail "GET /workspaces 缺新行"
code="$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$ADMIN/workspaces/docs" -H "Authorization: Bearer $TOKEN")"
[ "$code" = "200" ] || fail "DELETE /workspaces 期望 200 得 $code"
curl -s "$ADMIN/workspaces" -H "Authorization: Bearer $TOKEN" | grep -q '"docs"' && fail "删除后 GET 仍含 docs"
echo "[4] admin workspace CRUD OK"

echo "ACP-LOCAL-SMOKE-OK"
