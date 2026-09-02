#!/usr/bin/env bash
# p2p-cli smoke: bootstrap + 2 nodes, run discover and ping. Not in make check.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "${BASH_SOURCE[0]}")/.."
BIN="$(pwd)/target/debug/p2p-cli"
QPORT=34110
BOOT="127.0.0.1/u$QPORT"
TMP="$(mktemp -d)"; PIDS=()
cleanup(){ for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null||true; done; rm -rf "$TMP"; }
trap cleanup EXIT
wait_peer(){
  local f="$1" i; for i in $(seq 1 20); do
    local p; p="$(grep -oE "peer_id=[A-Za-z0-9]+" "$f" 2>/dev/null | head -1 | cut -d= -f2)";
    [ -n "$p" ] && { echo "$p"; return 0; }; sleep 1;
  done;
  echo "timeout waiting peer_id in $f" >&2; cat "$f" >&2; return 1;
}
start(){ "$BIN" "$@" >"$TMP/last.log" 2>&1 & PIDS+=("$!"); }
echo "[smoke] 1/5 build"; cargo build -p p2p-cli >/dev/null
echo "[smoke] 2/5 bootstrap"; "$BIN" bootstrap --data "$TMP/b" --listen-quic "127.0.0.1:$QPORT" --listen-tcp "127.0.0.1:$((QPORT+1))" >"$TMP/b.log" 2>&1 & PIDS+=("$!")
BPEER="$(wait_peer "$TMP/b.log")"; echo "[smoke] bootstrap peer_id=$BPEER"
echo "[smoke] 3/5 nodes"; "$BIN" node --data "$TMP/n1" --name a --bootstrap "$BOOT" >"$TMP/n1.log" 2>&1 & PIDS+=("$!")
"$BIN" node --data "$TMP/n2" --name b --bootstrap "$BOOT" >"$TMP/n2.log" 2>&1 & PIDS+=("$!")
N1PEER="$(wait_peer "$TMP/n1.log")"; echo "[smoke] node1 peer_id=$N1PEER"
echo "[smoke] 4/5 discover"; "$BIN" discover --bootstrap "$BOOT" --duration 8
echo "[smoke] 5/5 ping"; "$BIN" ping "$N1PEER" --bootstrap "$BOOT" --wait 12
echo "[smoke] PASS"
