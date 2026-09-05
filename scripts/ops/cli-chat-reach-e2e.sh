#!/usr/bin/env bash
# PR1 chat 可达性与行箱闭环 E2E（F1/F3/F4/F5/F12）六语义机械断言：
#   PORT-PERSIST  serve 端口记忆：A 随机端口重启沿用；B 显式端口落盘后重启沿用
#   ADDR-LEARN    B 换端口后经入站帧声明地址回写 A 好友簿
#   ADDR-EDIT     friends update --addr 整组替换（report JSON 与 list 双断言）
#   OUTBOX-FLUSH  serve 启动补投泵自动补投 + outbox flush 手动补投逐对端回报
#   PENDING-EXIT  离线 send 退出码 0 且 delivered=false/status=pending
#   INVITE-JSON   invites accept/reject/cancel 全 --json 单行可断言
# 另断言 F12 附件 mime 按扩展名（.txt -> text/plain）。
# fresh TMP 目录幂等可连跑两遍；造数当次清理（不过夜）。末行输出 PR1-REACH-OK。
# Not in make check；验收命令见任务书（协调者机械复跑，连跑两遍 + 门禁）。
set -eu
set -o pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
BIN="$(pwd)/apps/cli/target/debug/p2pctl"
[ -x "$BIN" ] || cargo build --manifest-path apps/cli/Cargo.toml >/dev/null

TMP="$(mktemp -d)"
PIDS=()
cleanup() {
  for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done
  sleep 0.5
  for p in "${PIDS[@]:-}"; do kill -9 "$p" 2>/dev/null || true; done
  rm -rf "$TMP"
}
trap cleanup EXIT

A_DIR="$TMP/a"; B_DIR="$TMP/b"; C_DIR="$TMP/c"
mkdir -p "$A_DIR" "$B_DIR" "$C_DIR"  # 身份锁与 chat 库要求数据目录已存在
step() { echo "[pr1-reach-e2e] $*"; }
last_pid() { echo "${PIDS[$((${#PIDS[@]}-1))]}"; }
pop_pid() { local n=$((${#PIDS[@]}-1)); [ "$n" -ge 0 ] || return 0; stop_serve "${PIDS[$n]}"; unset "PIDS[$n]"; }

# 等待 serve 就绪（stdout 首行 JSON 含 peerId/listenAddrs），输出 "peer uPort"。
wait_ready() {
  local f="$1" i peer uport
  for i in $(seq 1 20); do
    peer="$(sed -nE 's/.*"peerId":"([A-Za-z0-9]{40,60})".*/\1/p' "$f" 2>/dev/null | head -1)"
    uport="$(grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/u[0-9]+' "$f" 2>/dev/null | head -1 | sed 's|.*/u||')"
    if [ -n "$peer" ] && [ -n "$uport" ]; then echo "$peer $uport"; return 0; fi
    sleep 1
  done
  echo "timeout waiting serve ready: $f" >&2
  cat "$f" >&2
  return 1
}

READY=""
start_serve() {  # <data-dir> <out-file> [额外参数...]：后台起 serve（父 shell 记 PID），
  # 就绪行写入全局 READY（禁命令替换包本函数：子 shell 会丢 PIDS 数组）。
  local dir="$1" out="$2"
  shift 2
  "$BIN" chat serve --data-dir "$dir" --json "$@" >"$out" 2>"${out%.json}.err" &
  PIDS+=("$!")
  READY="$(wait_ready "$out")"
}

stop_serve() {  # <pid>：SIGTERM 并等身份锁释放
  local pid="$1"
  kill "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.2
  done
  wait "$pid" 2>/dev/null || true
}

uport_of() {  # 从 serve stdout JSON 提取 QUIC 端口
  grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/u[0-9]+' "$1" | head -1 | sed 's|.*/u||'
}

# 端口显式选取：冲突时 serve 起不来会显式超时失败（可重跑）。
P2=$((30000 + RANDOM % 20000))
P3=$((30000 + RANDOM % 20000))
if [ "$P3" = "$P2" ]; then P3=$((P2 + 1)); fi

step "1/12 start B serve on explicit port $P2"
start_serve "$B_DIR" "$TMP/b1.json" --quic-port "$P2"
B_PEER="${READY%% *}"
[ -n "$B_PEER" ] || { echo "no B peer" >&2; exit 1; }
[ "$(uport_of "$TMP/b1.json")" = "$P2" ] || { echo "B first port mismatch" >&2; exit 1; }
step "B peer=$B_PEER port=$P2"

step "2/12 start A serve (random port)"
start_serve "$A_DIR" "$TMP/a1.json"
A_PEER="${READY%% *}"; A_PORT="${READY##* }"
step "A peer=$A_PEER port=$A_PORT"

step "3/12 A one-shot: friend add + real delivery while B online"
pop_pid
"$BIN" chat friends add --data-dir "$A_DIR" "$B_PEER" --nickname B --addr "127.0.0.1/u$P2" --json \
  | grep -q '"delivered":true'
"$BIN" chat send --data-dir "$A_DIR" --peer "$B_PEER" --text a1-hello --json | grep -q '"delivered":true'

step "4/12 B accepts invite --json (INVITE-JSON accept)"
pop_pid
"$BIN" chat friends invites accept --data-dir "$B_DIR" "$A_PEER" --json | grep -q '"ok":true'
"$BIN" chat friends list --data-dir "$B_DIR" --json | grep -q "$A_PEER"
"$BIN" chat friends list --data-dir "$B_DIR" --json | grep -q "u$A_PORT"

step "5/12 B offline send -> exit 0 status=pending (PENDING-EXIT)"
set +e
"$BIN" chat send --data-dir "$B_DIR" --peer "$A_PEER" --text b1-auto --json >"$TMP/b1send.json" 2>"$TMP/b1send.err"
RC=$?
set -e
[ "$RC" -eq 0 ] || { echo "offline send must exit 0, got $RC" >&2; cat "$TMP/b1send.err" >&2; exit 1; }
grep -q '"delivered":false' "$TMP/b1send.json"
grep -q '"status":"pending"' "$TMP/b1send.json"
"$BIN" chat outbox list --data-dir "$B_DIR" --json | grep -q '"pending":1'
step "offline send queued: rc=0 delivered=false outbox pending=1"

step "6/12 A restart reuses random port (PORT-PERSIST)"
start_serve "$A_DIR" "$TMP/a2.json"
[ "${READY%% *}" = "$A_PEER" ] || { echo "A peer drifted after restart" >&2; exit 1; }
[ "${READY##* }" = "$A_PORT" ] || { echo "A port drifted: want $A_PORT got ${READY##* }" >&2; exit 1; }

step "7/12 B restart reuses explicit port $P2; startup sweeper auto-flushes b1"
start_serve "$B_DIR" "$TMP/b2.json"
[ "${READY##* }" = "$P2" ] || { echo "B port not persisted: got ${READY##* }" >&2; exit 1; }
A_ERR="$TMP/a2.err"
FOUND=""
for _ in $(seq 1 20); do
  if grep -q b1-auto "$A_ERR" 2>/dev/null; then FOUND=1; break; fi
  sleep 1
done
[ -n "$FOUND" ] || { echo "b1 not auto-flushed to A within 20s" >&2; tail -5 "$A_ERR" >&2; exit 1; }
step "b1 auto-flushed on B serve startup"

step "8/12 B moves to port $P3; inbound frame teaches A (ADDR-LEARN)"
pop_pid
start_serve "$B_DIR" "$TMP/b3.json" --quic-port "$P3"
[ "${READY##* }" = "$P3" ] || { echo "B explicit port $P3 not honored" >&2; exit 1; }
pop_pid
"$BIN" chat send --data-dir "$B_DIR" --peer "$A_PEER" --text b2-learn --json | grep -q '"delivered":true'
pop_pid
"$BIN" chat friends list --data-dir "$A_DIR" --json | grep -q "u$P3" \
  || { echo "A friend book did not learn new B port $P3" >&2; exit 1; }
step "A learned B new addr 127.0.0.1/u$P3"

step "9/12 A pending + manual flush delivers (OUTBOX-FLUSH manual)"
set +e
"$BIN" chat send --data-dir "$A_DIR" --peer "$B_PEER" --text a2-flush --json >"$TMP/a2send.json" 2>/dev/null
RC=$?
set -e
[ "$RC" -eq 0 ] || { echo "A offline send must exit 0, got $RC" >&2; exit 1; }
grep -q '"status":"pending"' "$TMP/a2send.json"
start_serve "$B_DIR" "$TMP/b4.json"
[ "${READY##* }" = "$P3" ] || { echo "B port persist broke after explicit move" >&2; exit 1; }
"$BIN" chat outbox flush --data-dir "$A_DIR" --json >"$TMP/flush.json"
grep -q '"flushedTotal":1' "$TMP/flush.json"
grep -q '"remainingTotal":0' "$TMP/flush.json"
pop_pid
"$BIN" chat history --data-dir "$B_DIR" --peer "$A_PEER" --json | grep -q a2-flush
step "manual flush: flushedTotal=1 remainingTotal=0, B history has a2-flush"

step "10/12 invites reject/cancel --json (INVITE-JSON)"
start_serve "$C_DIR" "$TMP/c1.json"
C_PEER="${READY%% *}"; C_PORT="${READY##* }"
# 先在 C 在线时送达邀请（否则 C 无来邀可拒），再停 C serve 走 one-shot 拒绝
"$BIN" chat friends add --data-dir "$A_DIR" "$C_PEER" --addr "127.0.0.1/u$C_PORT" --json \
  | grep -q '"delivered":true'
pop_pid
"$BIN" chat friends invites reject --data-dir "$C_DIR" "$A_PEER" --json | grep -q '"ok":true'
"$BIN" chat friends add --data-dir "$A_DIR" "$C_PEER" --addr "127.0.0.1/u$C_PORT" --json >/dev/null
"$BIN" chat friends invites cancel --data-dir "$A_DIR" "$C_PEER" --json | grep -q '"ok":true'

step "11/12 friends update --addr replaces addrs (ADDR-EDIT)"
"$BIN" chat friends update --data-dir "$A_DIR" "$B_PEER" --addr "127.0.0.1/u49999" --json >"$TMP/upd.json"
grep -q '"addrs":\["127.0.0.1/u49999"\]' "$TMP/upd.json" \
  || { echo "update report missing addrs" >&2; cat "$TMP/upd.json" >&2; exit 1; }
"$BIN" chat friends list --data-dir "$A_DIR" --json | grep -q 'u49999'
"$BIN" chat friends update --data-dir "$A_DIR" "$B_PEER" --addr "127.0.0.1/u$P3" --json >/dev/null
"$BIN" chat friends list --data-dir "$A_DIR" --json | grep -q "u$P3"
if "$BIN" chat friends list --data-dir "$A_DIR" --json | grep -q 'u49999'; then
  echo "addr replace left stale addr behind" >&2
  exit 1
fi

step "12/12 txt attachment sniffs text/plain (F12)"
start_serve "$B_DIR" "$TMP/b5.json"
[ "${READY##* }" = "$P3" ] || { echo "B port persist broken in step 12" >&2; exit 1; }
printf 'pr1-attachment-body' > "$TMP/note.txt"
"$BIN" chat send --data-dir "$A_DIR" --peer "$B_PEER" --kind file --file "$TMP/note.txt" --json \
  | grep -q '"delivered":true'
pop_pid
"$BIN" chat history --data-dir "$B_DIR" --peer "$A_PEER" --json | grep -q 'text/plain'

echo "PR1-REACH-OK"
