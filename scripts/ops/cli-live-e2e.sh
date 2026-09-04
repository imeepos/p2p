#!/usr/bin/env bash
# W1 E2E：CLI 写入 → GUI 实时感知（file-watch → data-changed → 前端定向重载）。
# 隔离：HOME 指向临时目录，GUI app 数据目录与 CLI --data-dir 同一目录；
#   不触碰真实用户数据；收尾清理进程与造数（不过夜）。
# 感知断言面：GUI 前端收到 data-changed 并定向重载后，向前端日志
#   frontend.log 追加 {"kind":"data-changed","domains":[...],"ts":...}（W1 前端
#   观测证据）；本脚本在 CLI 写入后 ≤3s 内轮询该文件断言 GUI 侧已感知，
#   并经 gui invoke 白名单只读命令读回断言与 CLI 写入一致。
# 构建钩子：W1_CTL / W1_GUI_BIN 可覆写产物路径；产物缺失或无监听能力
#   （二进制/前端产物不含 data-changed 标记）时现场重建，防旧产物假绿。
# 末行 W1-E2E-OK；两次连跑均绿为验收口径（防抖与轮询无跨跑状态）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="${W1_CTL:-$ROOT/apps/cli/target/debug/p2pctl}"
GUI_BIN="${W1_GUI_BIN:-$ROOT/apps/gui/src-tauri/target/debug/p2p-console}"
GUI_DIR="$ROOT/apps/gui"
GUI_LOG=""
TMPH=""
DATA=""
FLOG=""
CHILD=""

fail() {
  echo "W1-E2E FAIL: $1" >&2
  [ -n "$GUI_LOG" ] && { echo "--- GUI 日志末 20 行 ---" >&2; tail -20 "$GUI_LOG" >&2 || true; }
  exit 1
}

cleanup() {
  # 先摘 job 再杀：避免 bash 退出阶段对被杀后台任务回显 Terminated 噪声，
  # 保证末行始终是 W1-E2E-OK（对齐既有 E2E 末行口径）。
  [ -n "$CHILD" ] && disown "$CHILD" 2>/dev/null || true
  if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
    kill "$CHILD" 2>/dev/null || true
    for _ in $(seq 1 50); do kill -0 "$CHILD" 2>/dev/null || break; sleep 0.2; done
    kill -9 "$CHILD" 2>/dev/null || true
  fi
  [ -n "$TMPH" ] && rm -rf "$TMPH"
  return 0
}
trap cleanup EXIT

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

# 产物缺失或无监听能力时重建：前端产物不含 data-changed = 旧 bundle（无监听器）。
# GUI 一律 cargo build --features custom-protocol：debug 二进制默认加载
# devUrl(5173)，外部 vite dev server 占用端口时会装上旧 dev bundle；定制协议
# 特性强制内嵌 frontendDist，同时保证本跑产物一定包含最新前端。构建增量幂等。
build_if_missing() {
  if [ ! -x "$CTL" ]; then
    echo "p2pctl 缺失，构建 apps/cli…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2 || fail "CLI 构建失败"
  fi
  [ -n "${W1_GUI_BIN:-}" ] && return 0
  if [ ! -f "$GUI_DIR/dist/index.html" ] || ! grep -rq "data-changed" "$GUI_DIR/dist/assets" 2>/dev/null; then
    [ -d "$GUI_DIR/node_modules" ] || (cd "$GUI_DIR" && pnpm install --frozen-lockfile) >&2
    echo "前端产物缺失或无监听能力，pnpm build…" >&2
    (cd "$GUI_DIR" && pnpm build) >&2 || fail "前端构建失败"
  fi
  echo "GUI 构建（custom-protocol 内嵌产物）…" >&2
  (cd "$GUI_DIR/src-tauri" && cargo build --features custom-protocol) >&2 || fail "GUI 构建失败"
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

# watcher 必须真实在岗：启动成功日志在场，且无降级日志（R3 口径）。
assert_watcher_live() {
  grep -q "数据目录监听已启动" "$GUI_LOG" || fail "watcher 未启动（GUI 日志无启动行）"
  if grep -q "数据目录监听启动失败" "$GUI_LOG"; then
    fail "watcher 处于降级态（GUI 日志有失败行）"
  fi
  echo "watcher 在岗：数据目录监听已启动（config/profile/chat）"
}

# 前端感知链路就绪门：监听安装成功即落 data-watch-ready 标记行；
# 在此之前 CLI 写入的事件无人接收（webview 未装载完），必须等待后再开写。
wait_perception_ready() {
  for _ in $(seq 1 150); do
    if [ -f "$FLOG" ] && grep -q "data-watch-ready" "$FLOG" 2>/dev/null; then
      echo "前端感知链路就绪（data-watch-ready）"
      return 0
    fi
    kill -0 "$CHILD" 2>/dev/null || fail "GUI 进程提前退出（见 $GUI_LOG）"
    sleep 0.1
  done
  fail "15s 内前端感知链路未就绪（frontend.log 无 data-watch-ready）"
}

# frontend.log 中 data-changed 感知行计数（按域过滤）。
count_perception() {
  if [ -f "$FLOG" ]; then
    grep -c "\"data-changed\".*\"$1\"" "$FLOG" 2>/dev/null || true
  else
    echo 0
  fi
}

# ≤3s 感知断言：CLI 写入后轮询 frontend.log，新增感知行即 GUI 侧已感知。
# 整秒轮询上限取 2s，逐段校验 elapsed ≤3，保证不超任务书阈值。
wait_perception() {
  local domain="$1" base="$2" t0 n
  t0=$(date +%s)
  local deadline=$((SECONDS + 2))
  n="$(count_perception "$domain")"
  while [ "$n" -le "$base" ] && [ "$SECONDS" -lt "$deadline" ]; do
    sleep 0.1
    n="$(count_perception "$domain")"
  done
  local elapsed
  elapsed=$(( $(date +%s) - t0 ))
  [ "$n" -gt "$base" ] || fail "≤3s 内未感知 $domain 域写入（基线=$base 现值=$n）"
  [ "$elapsed" -le 3 ] || fail "$domain 感知耗时 ${elapsed}s 超过 3s"
  echo "GUI 侧已感知 $domain 域写入（${elapsed}s ≤ 3s）"
}

# 轮次一：CLI config save → 感知断言 + invoke 白名单只读命令读回一致。
round_config() {
  local cfg base
  base="$(count_perception "config")"
  cfg="{\"quicPort\":3400,\"tcpPort\":3401,\"enableMdns\":true,\"dataDir\":\"$DATA/p2p-data\",\"bootstrap\":[\"10.2.7.13/u3400\"],\"relayAddrs\":[\"10.2.7.13/u3403\"],\"advertisedAddrs\":[\"10.2.7.14/t4000\"],\"observationPort\":3402,\"observationAddrs\":[\"10.2.7.13:3402\"]}"
  printf "%s" "$cfg" | run_guarded "$CTL" config save - --data-dir "$DATA" >/dev/null
  wait_perception "config" "$base"
  run_guarded "$CTL" gui invoke config_get --gui-data-dir "$DATA" --json > "$TMPH/live-cfg-gui.json"
  run_guarded "$CTL" config get --data-dir "$DATA" --json > "$TMPH/live-cfg-cli.json"
  assert_json_equal "$TMPH/live-cfg-cli.json" "$TMPH/live-cfg-gui.json" 1 "config 读回（invoke 白名单只读命令 == CLI 写入）"
}

# 轮次二：CLI chat friends add → chat 域感知断言 + CLI 读回确认落盘。
round_chat() {
  local p1 base
  base="$(count_perception "chat")"
  p1=$(peer_id 1)
  run_guarded "$CTL" chat friends add "$p1" --nickname w1-live-1 --json --data-dir "$DATA" >/dev/null
  wait_perception "chat" "$base"
  run_guarded "$CTL" chat friends list --json --data-dir "$DATA" > "$TMPH/live-friends.json"
  grep -q "$p1" "$TMPH/live-friends.json" || fail "好友 $p1 未落盘（CLI 读回缺失）"
  echo "chat 好友簿写盘与 GUI 感知一致（$p1）"
}

# ---- 主流程 ----
build_if_missing
TMPH="$(mktemp -d "${TMPDIR:-/tmp}/w1-live-e2e.XXXXXX")"
DATA="$TMPH/Library/Application Support/com.p2p.console"
FLOG="$TMPH/Library/Logs/com.p2p.console/frontend.log"
mkdir -p "$DATA"

echo "== 1. 启动 GUI（隔离 HOME）并确认 watcher 与前端感知链路在岗 =="
start_gui_and_wait
assert_watcher_live
wait_perception_ready

echo "== 2. CLI config save → GUI ≤3s 实时感知 =="
round_config

echo "== 3. CLI chat friends add → GUI ≤3s 实时感知 =="
round_chat

echo "W1 实测结论：GUI 运行中 CLI 写 config/好友簿，前端经 data-changed 定向重载，"
echo "感知时延 ≤3s；invoke 白名单只读命令读回与 CLI 写入逐字段一致。"
echo "W1-E2E-OK"
