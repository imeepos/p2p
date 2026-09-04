#!/usr/bin/env bash
# CL3 chat 域 E2E：--data-dir 临时隔离；起 A/B 两节点 -> 建好友 -> 文本与附件
# 全链路 -> B 端 history/media 断言 -> 离线发送必须显式失败 -> friend remove
# 幂等断言。造数当次清理（不过夜）；连续执行两次必须绿（幂等）。
# 末行输出 CL3-E2E-OK。Not in make check.
set -euo pipefail
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

A_DIR="$TMP/a"; B_DIR="$TMP/b"
step() { echo "[cl3-e2e] $*"; }

# 等待 serve 就绪（stdout 首行 JSON 含 peerId/listenAddrs），输出 "peer tcpAddr"。
# 地址选 /t（TCP）：与 crates/p2p-itest chat 用例同约定，底座 chat 链路按 TCP 验证。
wait_ready() {
  local f="$1" i peer addr
  for i in $(seq 1 20); do
    peer="$(sed -nE 's/.*"peerId":"([A-Za-z0-9]{40,60})".*/\1/p' "$f" 2>/dev/null | head -1)"
    addr="$(grep -oE '"[0-9.]+/t[0-9]+"' "$f" 2>/dev/null | head -1 | tr -d '"')"
    if [ -n "$peer" ] && [ -n "$addr" ]; then echo "$peer $addr"; return 0; fi
    sleep 1
  done
  echo "timeout waiting serve ready: $f" >&2
  cat "$f" >&2
  return 1
}

step "1/8 start node A"
"$BIN" chat serve --data-dir "$A_DIR" --json >"$TMP/a.json" 2>"$TMP/a.log" &
PIDS+=("$!")
READY="$(wait_ready "$TMP/a.json")"
A_PEER="${READY%% *}"; A_ADDR="${READY##* }"
step "A peer=$A_PEER addr=$A_ADDR"

step "2/8 start node B"
"$BIN" chat serve --data-dir "$B_DIR" --json >"$TMP/b.json" 2>"$TMP/b.log" &
PIDS+=("$!")
READY="$(wait_ready "$TMP/b.json")"
B_PEER="${READY%% *}"; B_ADDR="${READY##* }"
step "B peer=$B_PEER addr=$B_ADDR"

step "3/8 friend add both ways + idempotency + list"
"$BIN" chat friends add --data-dir "$A_DIR" "$B_PEER" --nickname B --addr "$B_ADDR" --json | grep -q '"created":true'
"$BIN" chat friends add --data-dir "$A_DIR" "$B_PEER" --nickname B2 --addr "$B_ADDR" --json | grep -q '"created":false'
"$BIN" chat friends add --data-dir "$B_DIR" "$A_PEER" --nickname A --addr "$A_ADDR" --json | grep -q '"created":true'
"$BIN" chat friends list --data-dir "$A_DIR" --json | grep -q "$B_PEER"

step "4/8 send text A->B (real delivery)"
"$BIN" chat send --data-dir "$A_DIR" --peer "$B_PEER" --text "hello-cl3-你好" --json >"$TMP/send.json"
grep -q '"delivered":true' "$TMP/send.json"

step "5/8 B history asserts message visible"
"$BIN" chat history --data-dir "$B_DIR" --peer "$A_PEER" --json >"$TMP/bh.json"
grep -q 'hello-cl3' "$TMP/bh.json"
grep -q '"sender":"them"' "$TMP/bh.json"

step "6/8 send media A->B + media file path assert"
printf 'cl3-attachment-body' > "$TMP/note.txt"
"$BIN" chat send --data-dir "$A_DIR" --peer "$B_PEER" --kind file --file "$TMP/note.txt" --json >"$TMP/sendm.json"
grep -q '"delivered":true' "$TMP/sendm.json"
MSG_ID="$(sed -nE 's/.*"id":"([0-9a-f-]{36})".*/\1/p' "$TMP/sendm.json" | head -1)"
[ -n "$MSG_ID" ] || { echo "cannot extract media message id" >&2; cat "$TMP/sendm.json" >&2; exit 1; }
"$BIN" chat media file --data-dir "$B_DIR" --peer "$A_PEER" --message-id "$MSG_ID" --json >"$TMP/mf.json"
M_PATH="$(sed -nE 's/.*"path":"([^"]+)".*/\1/p' "$TMP/mf.json" | head -1)"
[ -n "$M_PATH" ] || { echo "cannot extract media path" >&2; cat "$TMP/mf.json" >&2; exit 1; }
grep -q 'cl3-attachment-body' "$M_PATH"

step "7/8 offline send must fail loudly (exit 1, no silent success)"
FAKE_PEER="$(printf '1%.0s' {1..32})"
if "$BIN" chat send --data-dir "$A_DIR" --peer "$FAKE_PEER" --text x --timeout-secs 8 \
    --json >"$TMP/off.json" 2>"$TMP/off.err"; then
  echo "offline send unexpectedly succeeded" >&2
  cat "$TMP/off.json" >&2
  exit 1
fi
grep -q "未送达\|超时\|失败" "$TMP/off.err" "$TMP/off.json"

step "8/8 friend remove changes list (idempotent)"
"$BIN" chat friends remove --data-dir "$A_DIR" "$B_PEER" --json | grep -q '"removed":true'
if "$BIN" chat friends list --data-dir "$A_DIR" --json | grep -q "$B_PEER"; then
  echo "friend still listed after remove" >&2
  exit 1
fi
"$BIN" chat friends remove --data-dir "$A_DIR" "$B_PEER" --json | grep -q '"removed":false'

step "8b friend group roundtrip (IM-T43: --group / update / ungrouped)"
"$BIN" chat friends add --data-dir "$A_DIR" "$B_PEER" --nickname B3 --group 同事 --json \
  | grep -q '"group":"同事"'
"$BIN" chat friends list --data-dir "$A_DIR" --group 同事 --json | grep -q "$B_PEER"
if "$BIN" chat friends list --data-dir "$A_DIR" --group 家人 --json | grep -q "$B_PEER"; then
  echo "group filter leaked across groups" >&2
  exit 1
fi
"$BIN" chat friends update --data-dir "$A_DIR" "$B_PEER" --group "" --json | grep -q '"group":null'
"$BIN" chat friends list --data-dir "$A_DIR" --group "" --json | grep -q "$B_PEER"
if "$BIN" chat friends list --data-dir "$A_DIR" --group "" --json | grep -q '"group":"'; then
  echo "ungrouped filter must not contain named-group rows" >&2
  exit 1
fi
# 越界组名必须可读拒绝且不落盘
if "$BIN" chat friends update --data-dir "$A_DIR" "$B_PEER" --group "$(printf '组%.0s' {1..33})" \
    --json >"$TMP/badgrp.json" 2>&1; then
  echo "oversized group name unexpectedly accepted" >&2
  cat "$TMP/badgrp.json" >&2
  exit 1
fi
grep -q "分组名超过" "$TMP/badgrp.json"

for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done
echo "CL3-E2E-OK"