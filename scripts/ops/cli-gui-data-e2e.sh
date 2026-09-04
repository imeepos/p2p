#!/usr/bin/env bash
# N2 E2E：GUI 与 CLI 同数据目录的数据面互操作（冷一致性 + 运行中实时性语义）。
# 隔离：HOME 指向临时目录 → GUI app 数据目录（config/profile/chat/control）整体
#   落入临时盘，CLI --data-dir 指向同一目录；不触碰真实用户数据。
# 流程：构建产物（缺失才建）→ ① CLI 冷写 config/profile/chat → 启动 GUI →
#   经 p2pctl gui invoke 白名单只读命令读回，断言与 CLI 写入一致 →
#   ② GUI 侧写探测：白名单刻意只收只读命令，写命令 INVOKE_FORBIDDEN，
#   缺口记录（不阻断，详见 docs/ops/cli-guide.md §9）→ ③ R3 实测：GUI 运行中
#   CLI 再写 config/profile，invoke 立即读得新值（每调用直读磁盘）→
#   R2 双流并发写子脚本（cli-chat-concurrency-e2e.sh）→ 清理进程与造数
#   （不过夜）→ 末行 N2-E2E-OK。
# 测试钩子：N2_CTL / N2_GUI_BIN 可覆写产物路径（默认仓库内相对路径）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="${N2_CTL:-$ROOT/apps/cli/target/debug/p2pctl}"
GUI_BIN="${N2_GUI_BIN:-$ROOT/apps/gui/src-tauri/target/debug/p2p-console}"
GUI_DIR="$ROOT/apps/gui"
GUI_LOG=""
TMPH=""
DATA=""
CHILD=""

fail() {
  echo "N2-E2E FAIL: $1" >&2
  [ -n "$GUI_LOG" ] && { echo "--- GUI 日志末 20 行 ---" >&2; tail -20 "$GUI_LOG" >&2 || true; }
  exit 1
}

cleanup() {
  if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
    kill "$CHILD" 2>/dev/null || true
    for _ in $(seq 1 50); do kill -0 "$CHILD" 2>/dev/null || break; sleep 0.2; done
    kill -9 "$CHILD" 2>/dev/null || true
  fi
  [ -n "$TMPH" ] && rm -rf "$TMPH"
  return 0
}
trap cleanup EXIT

# 跑一步原语；非零先原样回显错误输出（失败可见）再终止。
run_guarded() {
  local out rc
  set +e
  out="$("$@" 2>&1)"
  rc=$?
  set -e
  if [ "$rc" -ne 0 ]; then printf "%s\n" "$out" >&2; fail "步骤失败（rc=$rc）: $*"; fi
  printf "%s" "$out"
}

peer_id() {
  python3 -c '
import sys
A = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
n = int.from_bytes(bytes([int(sys.argv[1])]) * 32, "big")
s = ""
while n:
    n, r = divmod(n, 58)
    s = A[r] + s
print(s)
' "$1"
}

# 深比较两份 JSON；unwrap_gui=1 时右侧先剥 {"result":…} 包装。
assert_json_equal() {
  python3 - "$1" "$2" "${3:-0}" "$4" <<'PY'
import json, sys
a = json.load(open(sys.argv[1]))
b = json.load(open(sys.argv[2]))
if sys.argv[3] == "1":
    b = b["result"]
assert a == b, (sys.argv[4] + " 不一致:\n  cli="
                + json.dumps(a, ensure_ascii=False) + "\n  gui="
                + json.dumps(b, ensure_ascii=False))
print(sys.argv[4] + " 一致")
PY
}

build_if_missing() {
  if [ ! -x "$CTL" ]; then
    echo "p2pctl 缺失，构建 apps/cli…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2 || fail "CLI 构建失败"
  fi
  [ -n "${N2_GUI_BIN:-}" ] && return 0
  if [ ! -f "$GUI_DIR/dist/index.html" ]; then
    [ -d "$GUI_DIR/node_modules" ] || (cd "$GUI_DIR" && pnpm install --frozen-lockfile) >&2
    echo "前端产物缺失，pnpm build…" >&2
    (cd "$GUI_DIR" && pnpm build) >&2 || fail "前端构建失败"
  fi
  if [ ! -x "$GUI_BIN" ]; then
    echo "GUI 二进制缺失，cargo build src-tauri…" >&2
    (cd "$GUI_DIR/src-tauri" && cargo build) >&2 || fail "GUI 构建失败"
  fi
  [ -x "$GUI_BIN" ] || fail "GUI 二进制未就绪: $GUI_BIN"
}

# 启动隔离 HOME 的真实 GUI，轮询端点文件（pid 匹配）与 gui status 就绪。
start_gui_and_wait() {
  GUI_LOG="$TMPH/gui.log"
  HOME="$TMPH" "$GUI_BIN" >>"$GUI_LOG" 2>&1 &
  CHILD=$!
  local ep="$DATA/control/endpoint.json"
  for _ in $(seq 1 90); do
    if [ -f "$ep" ] && grep -Eq "\"pid\": $CHILD([^0-9]|$)" "$ep" \
      && "$CTL" gui status --gui-data-dir "$DATA" --json >/dev/null 2>&1; then
      echo "GUI 就绪 pid=$CHILD 数据目录=$DATA"
      return 0
    fi
    kill -0 "$CHILD" 2>/dev/null || fail "GUI 进程提前退出（见 $GUI_LOG）"
    sleep 1
  done
  fail "GUI 端点 90s 未就绪（pid=$CHILD）"
}

# R1① 前半：GUI 启动前 CLI 冷写三域（config / profile / chat 好友簿）。
cold_cli_writes() {
  local cfg pr p1 p2 p3
  cfg="{\"quicPort\":3400,\"tcpPort\":3401,\"enableMdns\":true,\"dataDir\":\"$DATA/p2p-data\",\"bootstrap\":[\"10.2.7.13/u3400\"],\"relayAddrs\":[\"10.2.7.13/u3403\"],\"advertisedAddrs\":[\"10.2.7.14/t4000\"],\"observationPort\":3402,\"observationAddrs\":[\"10.2.7.13:3402\"]}"
  printf "%s" "$cfg" | run_guarded "$CTL" config save - --data-dir "$DATA" >/dev/null
  pr="{\"name\":\"n2-cold-节点\",\"description\":\"N2 冷写资料\",\"avatar\":null}"
  printf "%s" "$pr" | run_guarded "$CTL" profile save - --data-dir "$DATA" >/dev/null
  p1=$(peer_id 1); p2=$(peer_id 2); p3=$(peer_id 3)
  run_guarded "$CTL" chat friends add "$p1" --nickname n2-cold-1 --json --data-dir "$DATA" >/dev/null
  run_guarded "$CTL" chat friends add "$p2" --nickname n2-cold-2 --json --data-dir "$DATA" >/dev/null
  run_guarded "$CTL" chat friends add "$p3" --nickname n2-cold-3 --json --data-dir "$DATA" >/dev/null
  echo "CLI 冷写完成：config/profile 落盘 + 好友 3 笔"
}

# R1① 后半：GUI 起后经 invoke 白名单读回，与 CLI 读路径逐字段一致。
assert_cold_readback() {
  run_guarded "$CTL" gui invoke config_get --gui-data-dir "$DATA" --json > "$TMPH/gui-cfg.json"
  run_guarded "$CTL" config get --data-dir "$DATA" --json > "$TMPH/cli-cfg.json"
  assert_json_equal "$TMPH/cli-cfg.json" "$TMPH/gui-cfg.json" 1 "config 冷读回（CLI 写 → GUI 读）"
  run_guarded "$CTL" gui invoke profile_get --gui-data-dir "$DATA" --json > "$TMPH/gui-prof.json"
  run_guarded "$CTL" profile get --data-dir "$DATA" --json > "$TMPH/cli-prof.json"
  assert_json_equal "$TMPH/cli-prof.json" "$TMPH/gui-prof.json" 1 "profile 冷读回（CLI 写 → GUI 读）"
  run_guarded "$CTL" chat friends list --json --data-dir "$DATA" > "$TMPH/cli-friends.json"
  assert_json_equal "$TMPH/cli-friends.json" "$DATA/chat/friends.json" 0 "chat 好友簿（CLI 读 == 磁盘）"
  echo "缺口：invoke 白名单无 chat 只读命令，GUI 侧 chat 视图一致性未覆盖（docs §9 登记）"
}

# R1② GUI 侧写探测：白名单红线「写命令永不入列」→ INVOKE_FORBIDDEN。
assert_gui_write_gap() {
  local out rc
  set +e
  out=$("$CTL" gui invoke config_save --gui-data-dir "$DATA" 2>&1)
  rc=$?
  set -e
  [ "$rc" -ne 0 ] || fail "config_save 竟被白名单放行"
  printf "%s" "$out" | grep -q "INVOKE_FORBIDDEN" || fail "config_save 拒绝缺结构化码: $out"
  set +e
  out=$("$CTL" gui invoke chat_friend_add --gui-data-dir "$DATA" 2>&1)
  rc=$?
  set -e
  [ "$rc" -ne 0 ] || fail "chat_friend_add 竟被白名单放行"
  printf "%s" "$out" | grep -q "INVOKE_FORBIDDEN" || fail "chat_friend_add 拒绝缺结构化码: $out"
  echo "缺口：GUI 侧写（config_save/chat_friend_add 等）全部被 invoke 白名单拒绝，"
  echo "      GUI 写 → CLI 读回无法经控制通道验证（刻意红线，docs §9 登记，不阻断）"
}

# R3 实测：GUI 运行中 CLI 再写，invoke 白名单读路径即时可见（每调用直读磁盘）。
live_write_semantics() {
  local cfg pr p4
  cfg="{\"quicPort\":3401,\"tcpPort\":3401,\"enableMdns\":true,\"dataDir\":\"$DATA/p2p-data\",\"bootstrap\":[\"10.2.7.13/u3400\"],\"relayAddrs\":[\"10.2.7.13/u3403\"],\"advertisedAddrs\":[\"10.2.7.14/t4000\"],\"observationPort\":3402,\"observationAddrs\":[\"10.2.7.13:3402\"]}"
  printf "%s" "$cfg" | run_guarded "$CTL" config save - --data-dir "$DATA" >/dev/null
  run_guarded "$CTL" gui invoke config_get --gui-data-dir "$DATA" --json > "$TMPH/live-cfg.json"
  run_guarded "$CTL" config get --data-dir "$DATA" --json > "$TMPH/live-cli.json"
  assert_json_equal "$TMPH/live-cli.json" "$TMPH/live-cfg.json" 1 "R3 运行中 config 改写即时读回"
  pr="{\"name\":\"n2-live-改\",\"description\":\"N2 运行中改写\",\"avatar\":null}"
  printf "%s" "$pr" | run_guarded "$CTL" profile save - --data-dir "$DATA" >/dev/null
  run_guarded "$CTL" gui invoke profile_get --gui-data-dir "$DATA" --json > "$TMPH/live-prof.json"
  run_guarded "$CTL" profile get --data-dir "$DATA" --json > "$TMPH/live-prof-cli.json"
  assert_json_equal "$TMPH/live-prof-cli.json" "$TMPH/live-prof.json" 1 "R3 运行中 profile 改写即时读回"
  echo "R3 实测结论：GUI 运行中 CLI 写 config/profile，invoke 只读命令即时可见"
  echo "（GUI 侧 config_get/profile_get 每调用直读磁盘，无需刷新或重启）"
  p4=$(peer_id 4)
  run_guarded "$CTL" chat friends add "$p4" --nickname n2-live-4 --json --data-dir "$DATA" >/dev/null
  echo "R3 实测结论：运行中 CLI 写 chat 好友簿同样即时落盘（GUI chat 视图受白名单限制未断言）"
}

# R2：双流并发写一致性（子脚本自含断言，末行 N2-R2-OK）。
run_concurrency() {
  local out
  out="$TMPH/r2.log"
  N2_CTL="$CTL" bash "$ROOT/scripts/ops/cli-chat-concurrency-e2e.sh" > "$out" 2>&1 \
    || { cat "$out" >&2; fail "并发写子脚本失败"; }
  cat "$out"
  [ "$(tail -1 "$out")" = "N2-R2-OK" ] || fail "并发写子脚本末行非 N2-R2-OK"
}

# ---- 主流程 ----
build_if_missing
TMPH="$(mktemp -d "${TMPDIR:-/tmp}/n2-data-e2e.XXXXXX")"
DATA="$TMPH/Library/Application Support/com.p2p.console"
mkdir -p "$DATA"

echo "== 1. CLI 冷写（GUI 未启动，临时 HOME 隔离） =="
cold_cli_writes

echo "== 2. 启动 GUI（同一数据目录）并就绪轮询 =="
start_gui_and_wait

echo "== 3. 冷一致性读回（CLI 写 → GUI invoke 读） =="
assert_cold_readback

echo "== 4. GUI 侧写缺口探测（白名单只读红线） =="
assert_gui_write_gap

echo "== 5. R3 实测：GUI 运行中 CLI 写入的感知语义 =="
live_write_semantics

echo "== 6. R2 并发写一致性（独立临时数据目录） =="
run_concurrency

echo "N2-E2E-OK"
