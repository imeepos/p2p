#!/usr/bin/env bash
# INTEROP-IV1 smoke: real two-process interop run.
#   p2pctl node (rendezvous/ping side) <-> examples/mininode-python (third-party node)
# Steps: build p2pctl if needed -> venv + deps -> golden-vector selftest ->
#   start node daemon (random high ports) -> mininode register/query/ping ->
#   assert markers -> cleanup by PID (no pkill).
# Any failure exits non-zero with a reason on stderr.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
P2PCTL="$ROOT/apps/cli/target/debug/p2pctl"
PY_DIR="$ROOT/examples/mininode-python"
VENV="$ROOT/.venv-interop"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/mininode-smoke.XXXXXX")"
NODE_DIR="$TMP/node"
NAMESPACE="iv1-smoke-$$-$RANDOM"
NODE_PID=""
FAILED=""

log() { echo "[smoke $(date +%H:%M:%S)] $*"; }
die() { echo "[smoke FAIL] $*" >&2; FAILED=1; cleanup; exit 1; }

cleanup() {
  if [ -n "$NODE_PID" ] && kill -0 "$NODE_PID" 2>/dev/null; then
    log "stopping node daemon pid=$NODE_PID"
    kill "$NODE_PID" 2>/dev/null || true
    sleep 1
    kill -9 "$NODE_PID" 2>/dev/null || true
  fi
  "$P2PCTL" node stop --data-dir "$NODE_DIR" >/dev/null 2>&1 || true
  log "artifacts kept at $TMP"
}

trap cleanup EXIT

[ -x "$P2PCTL" ] || {
  log "p2pctl missing, building (first run can take minutes)"
  cargo build --manifest-path "$ROOT/apps/cli/Cargo.toml" 2>&1 | tail -1 || die "cargo build failed"
}
[ -x "$P2PCTL" ] || die "p2pctl binary not found at $P2PCTL"

log "preparing python venv"
if [ ! -x "$VENV/bin/python" ]; then
  python3 -m venv "$VENV" || die "venv creation failed"
  "$VENV/bin/pip" install --quiet --upgrade pip || die "pip upgrade failed"
fi
"$VENV/bin/pip" install --quiet -r "$PY_DIR/requirements.txt" \
  || die "pip install -r requirements.txt failed"

log "step 1/4 golden-vector selftest (varint/frame/peer-id/rendezvous)"
("$VENV/bin/python" "$PY_DIR/selftest.py" "$ROOT/docs/protocol/vectors" \
  | tail -1 | grep -q "SELFTEST-OK") || die "selftest against golden vectors failed"

log "step 2/4 starting p2pctl node daemon (lan-only, random ports)"
mkdir -p "$NODE_DIR"
printf '%s' '{"quicPort":0,"tcpPort":0,"enableMdns":false,"dataDir":"'"$NODE_DIR"'/p2p-data","bootstrap":[],"relayAddrs":[],"advertisedAddrs":[],"lanOnly":true}' \
  | "$P2PCTL" config save --data-dir "$NODE_DIR" - >/dev/null \
  || die "config save failed"
"$P2PCTL" node start --data-dir "$NODE_DIR" --json > "$TMP/start.json" 2> "$TMP/start.err" \
  || die "node start failed: $(cat "$TMP/start.err")"
read -r NODE_PEER NODE_PORT < <(
  "$VENV/bin/python" - "$TMP/start.json" <<'PYEOF'
import json, sys
with open(sys.argv[1]) as fh:
    doc = json.load(fh)
addrs = [a for a in doc["listenAddrs"] if a.startswith("127.0.0.1/u")]
print(doc["peerId"], addrs[0].split("/u")[1])
PYEOF
) || die "cannot parse node start output (peer/QUIC port)"
[ -f "$NODE_DIR/daemon.pid" ] && NODE_PID="$(cat "$NODE_DIR/daemon.pid")"
log "node up: peer=$NODE_PEER quic=127.0.0.1/u$NODE_PORT pid=$NODE_PID"

log "step 3/4 mininode: dial + signed register + precise query + ping"
MININODE_OUT="$TMP/mininode.out"
if ! "$VENV/bin/python" "$PY_DIR/mininode.py" \
    --seed "$TMP/mininode-seed" \
    --listen 127.0.0.1:0 \
    --bootstrap "127.0.0.1/u$NODE_PORT" \
    --peer "$NODE_PEER" \
    --namespace "$NAMESPACE" \
    --ttl-secs 60 \
    > "$MININODE_OUT" 2> "$TMP/mininode.err"; then
  sed 's/^/    | /' "$TMP/mininode.err" >&2
  die "mininode run failed (see stderr above; node log: $NODE_DIR/daemon.log)"
fi
sed 's/^/    | /' "$MININODE_OUT"

log "step 4/4 asserting interop markers"
grep -q "REGISTER-OK" "$MININODE_OUT" || die "register did not succeed"
grep -q "QUERY-OK" "$MININODE_OUT" || die "query did not succeed"
grep -q "PING-OK" "$MININODE_OUT" || die "ping roundtrip did not succeed"
grep -q "MININODE-OK" "$MININODE_OUT" || die "mininode did not complete cleanly"

log "ALL-GREEN: identity/QUIC-mTLS/rendezvous-register/query/ping interoperated"
log "SMOKE-OK namespace=$NAMESPACE"
exit 0
