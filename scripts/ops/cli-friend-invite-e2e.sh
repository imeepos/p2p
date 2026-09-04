#!/usr/bin/env bash
# 邀请制加好友 CLI E2E（IM 邀请流）：邀请送达 -> 对方同意 -> 双向互为好友
# -> 昵称各侧自治 -> 已是好友再邀请拒绝 -> 好友私聊互通。
# 编排纪律：
#   1) D6 身份互斥：同 data-dir 同一时刻至多一个进程，serve 与一次性命令轮转；
#   2) 双侧 serve 均用固定 --quic-port：INVITE 帧携带 advertised 声明地址，
#      地址稳定是 accept 回投收敛的前提；
#   3) 对端进程重启可清除半开连接残留（同 peerId 重连会被残留挡下，见 ISSUE.md）。
# 末行输出 FRIEND-INVITE-CLI-E2E-OK。Not in make check（opt-in）。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/../.."
BIN="$(pwd)/apps/cli/target/debug/p2pctl"
[ -x "$BIN" ] || cargo build --manifest-path apps/cli/Cargo.toml >/dev/null

TMP="$(mktemp -d)"
PA=""; PB=""; PB2=""
cleanup() {
  for p in "$PA" "$PB" "$PB2"; do
    if [ -n "$p" ]; then kill "$p" 2>/dev/null || true; fi
  done
  sleep 0.5
  for p in "$PA" "$PB" "$PB2"; do
    if [ -n "$p" ]; then kill -9 "$p" 2>/dev/null || true; fi
  done
  rm -rf "$TMP"
}
trap cleanup EXIT

A_DIR="$TMP/a"; B_DIR="$TMP/b"
mkdir -p "$A_DIR" "$B_DIR"
step() { echo "[invite-e2e] $*"; }

read_field() {
  python3 -c "import sys,json;print(json.load(sys.stdin)[sys.argv[1]])" "$1"
}

read_field_first() {
  python3 -c "import sys,json;print(json.load(sys.stdin)[sys.argv[1]][0])" "$1"
}

json_has() {
  python3 -c "import sys,json;needle,key=sys.argv[1],sys.argv[2];items=json.load(sys.stdin);sys.exit(0 if any(i.get(key)==needle for i in items) else 1)" "$1" "$2"
}

wait_ready() {
  local f="$1" i peer addr
  for i in $(seq 1 20); do
    if [ -s "$f" ]; then
      peer="$(cat "$f" | read_field peerId)"
      addr="$(cat "$f" | read_field_first listenAddrs)"
      if [ -n "$peer" ] && [ -n "$addr" ]; then echo "$peer $addr"; return 0; fi
    fi
    sleep 0.5
  done
  echo "timeout waiting serve ready: $f" >&2
  return 1
}

step "1/9 start node B (fixed port 41002, stays up)"
"$BIN" chat serve --data-dir "$B_DIR" --quic-port 41002 --json >"$TMP/b.json" 2>/dev/null &
PB=$!
READY="$(wait_ready "$TMP/b.json")"
B_PEER="$(echo "$READY" | awk '{print $1}')"
B_ADDR="$(echo "$READY" | awk '{print $2}')"
step "B peer=$B_PEER addr=$B_ADDR"

step "2/9 A sends invite (delivered=true)"
"$BIN" chat friends add --data-dir "$A_DIR" "$B_PEER" --nickname 小b --addr "$B_ADDR" --json >"$TMP/add.json" 2>"$TMP/add.err" || { echo "FAIL invite command" >&2; cat "$TMP/add.json" "$TMP/add.err" >&2; exit 1; }
if [ "$(cat "$TMP/add.json" | read_field delivered)" != "True" ]; then echo "FAIL invite not delivered" >&2; cat "$TMP/add.json" >&2; exit 1; fi
step "OK: invite delivered"

step "3/9 consent required: A friend list still empty"
"$BIN" chat friends list --data-dir "$A_DIR" --json | grep -qxF "[]" || { echo "FAIL friend list not empty before consent"; exit 1; }
step "OK: no friendship before consent"

step "4/9 swap: B serve down, A serve up (fixed port 41001), heal refreshes B"
"$BIN" chat serve --data-dir "$A_DIR" --quic-port 41001 --json >"$TMP/a.json" 2>/dev/null &
PA=$!
READY="$(wait_ready "$TMP/a.json")"
A_PEER="$(echo "$READY" | awk '{print $1}')"
"$BIN" chat friends invites list --data-dir "$B_DIR" --json | json_has in direction || { echo "FAIL B has no incoming invite"; exit 1; }
step "OK: B holds incoming invite"

step "5/9 B accepts (reply dials advertised addr 41001)"
HEALED=""
for i in $(seq 1 30); do
  if "$BIN" chat friends invites list --data-dir "$B_DIR" --json | grep -qF "u41001"; then HEALED=1; break; fi
  sleep 0.5
done
if [ -z "$HEALED" ]; then echo "FAIL B never learned A addr"; exit 1; fi
if [ -n "$PB" ]; then kill "$PB"; wait "$PB" 2>/dev/null; PB=""; fi
"$BIN" chat friends invites accept --data-dir "$B_DIR" "$A_PEER" --nickname 阿a >/dev/null || { echo "FAIL accept" >&2; exit 1; }
step "OK: accepted"

step "6/9 mutual friendship on both sides"
"$BIN" chat friends list --data-dir "$B_DIR" --json | json_has "$A_PEER" peerId || { echo "FAIL B side missing friend"; exit 1; }
FOUND=""
for i in $(seq 1 20); do
  if "$BIN" chat friends list --data-dir "$A_DIR" --json | json_has "$B_PEER" peerId; then FOUND=1; break; fi
  sleep 0.5
done
if [ -z "$FOUND" ]; then echo "FAIL A side missing friend (ACCEPT not converged)"; exit 1; fi
step "OK: mutual friends"

step "7/9 nickname autonomy (A set xiaob, B set a-a)"
"$BIN" chat friends list --data-dir "$A_DIR" --json | json_has 小b nickname || { echo "FAIL A nickname"; exit 1; }
"$BIN" chat friends list --data-dir "$B_DIR" --json | json_has 阿a nickname || { echo "FAIL B nickname"; exit 1; }
step "OK: nicknames"

step "8/9 invite to existing friend must fail loudly (B side, identity idle)"
if "$BIN" chat friends add --data-dir "$B_DIR" "$A_PEER" --nickname x --addr "$B_ADDR" >/dev/null 2>"$TMP/dup.err"; then
  echo "FAIL duplicate invite unexpectedly succeeded" >&2; exit 1
fi
grep -q "已是好友" "$TMP/dup.err"
step "OK: already-friends rejected"

step "9/9 friend chat works: restart B (clear stale), A sends"
if [ -n "$PA" ]; then kill "$PA"; wait "$PA" 2>/dev/null; PA=""; fi
if [ -n "$PB" ]; then kill "$PB"; wait "$PB" 2>/dev/null; PB=""; fi
"$BIN" chat serve --data-dir "$B_DIR" --quic-port 41002 --json >"$TMP/b2.json" 2>/dev/null &
PB2=$!
sleep 2
"$BIN" chat send --data-dir "$A_DIR" --peer "$B_PEER" --text 你好啊 --json >"$TMP/send.json" 2>"$TMP/send.err" || { echo "FAIL send" >&2; cat "$TMP/send.json" "$TMP/send.err" >&2; exit 1; }
if [ "$(cat "$TMP/send.json" | read_field delivered)" != "True" ]; then echo "FAIL chat not delivered" >&2; cat "$TMP/send.json" >&2; exit 1; fi
if [ -n "$PB2" ]; then kill "$PB2"; wait "$PB2" 2>/dev/null; PB2=""; fi
"$BIN" chat history --data-dir "$B_DIR" --peer "$A_PEER" --json | grep -q '你好啊' || { echo "FAIL B history missing message"; exit 1; }
step "OK: chat delivered"

echo "FRIEND-INVITE-CLI-E2E-OK"
