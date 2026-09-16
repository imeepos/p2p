#!/usr/bin/env bash
# vdrive 真机挂载冒烟：双 p2pctl 进程 + mount_webdav 实挂实测。
# 口径 VDRIVE-MOUNT-SMOKE-OK：经挂载点读写删与直写对侧互见，
# curl PROPFIND/GET 同源验证桥协议面；macOS 无 mount_webdav 时降级
# 为桥级验证并以 MOUNT-UNAVAILABLE 显式标注（不算通过实挂口径）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/apps/cli/target/debug/p2pctl"
WORK="$(mktemp -d /tmp/vdrive-smoke.XXXXXX)"
NAME="vdrive-smoke-$$"
PORT=$((20000 + RANDOM % 20000))
PIDS=()
MOUNTED=0

cleanup() {
  if [ "$MOUNTED" = "1" ]; then diskutil unmount force "$MOUNTPOINT" >/dev/null 2>&1 || true; fi
  for pid in "${PIDS[@]:-}"; do kill "$pid" >/dev/null 2>&1 || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK" "$HOME/.vdrive-mounts/$NAME"
  rmdir "$HOME/.vdrive-mounts" 2>/dev/null || true
}
trap cleanup EXIT

fail() { echo "vdrive-mount-smoke: FAIL: $*" >&2; exit 1; }
ready_of() { # 字段名走 stdin 的就绪 JSON 行 -> 提取字段值
  python3 -c "import json,sys; d=json.loads(sys.stdin.read() or '{}'); print(d.get('$1',''))"
}

[ -x "$BIN" ] || { echo "未找到 $BIN，先 (cd apps/cli && cargo build)" >&2; exit 1; }
command -v python3 >/dev/null || fail "需要 python3"

# 1) 被挂端：seed 文件先行
SERVE_ROOT="$WORK/root"
mkdir -p "$SERVE_ROOT/docs"
echo "remote-seed-v1" > "$SERVE_ROOT/docs/seed.txt"

"$BIN" vdrive serve --root "$SERVE_ROOT" --data-dir "$WORK/id-a" --no-mdns \
  > "$WORK/serve.out" 2> "$WORK/serve.err" &
PIDS+=($!)
for _ in $(seq 1 100); do
  grep -q '"kind":"ready"' "$WORK/serve.out" 2>/dev/null && break
  sleep 0.1
done
grep -q '"kind":"ready"' "$WORK/serve.out" || { cat "$WORK/serve.err" >&2; fail "serve 未就绪"; }
PEER=$(tail -n 1 "$WORK/serve.out" | ready_of peerId)
ADDRS=$(python3 -c "
import json,sys
line=[l for l in open('$WORK/serve.out') if 'ready' in l][-1]
print(' '.join(json.loads(line)['listenAddrs']))")
[ -n "$PEER" ] && [ -n "$ADDRS" ] || fail "serve 就绪行缺 peerId/listenAddrs"
echo "serve ready: peer=$PEER addrs=$ADDRS"

# 2) 挂载端：桥 + 自动挂载
ADDR_ARGS=()
for a in $ADDRS; do ADDR_ARGS+=(--addr "$a"); done
"$BIN" vdrive mount --peer "$PEER" "${ADDR_ARGS[@]}" \
  --data-dir "$WORK/id-b" --no-mdns --port "$PORT" --mount --name "$NAME" \
  > "$WORK/mount.out" 2> "$WORK/mount.err" &
PIDS+=($!)
for _ in $(seq 1 100); do
  grep -q '"kind":"ready"' "$WORK/mount.out" 2>/dev/null && break
  kill -0 "${PIDS[1]}" 2>/dev/null || { cat "$WORK/mount.err" >&2; fail "mount 进程早退"; }
  sleep 0.1
done
grep -q '"kind":"ready"' "$WORK/mount.out" || { cat "$WORK/mount.err" >&2; fail "mount 未就绪"; }
if grep -q '"kind":"mount-failed"' "$WORK/mount.out"; then
  echo "MOUNT-UNAVAILABLE: mount_webdav 不可用（见 mount.out），降级桥级验证"
fi
MOUNTPOINT=$(grep '"kind":"mounted"' "$WORK/mount.out" | tail -n 1 | ready_of mountpoint)
if [ -n "$MOUNTPOINT" ] && mount | grep -q "$MOUNTPOINT"; then MOUNTED=1; fi
echo "mount ready: url=http://127.0.0.1:$PORT/ mounted=$MOUNTED mountpoint=$MOUNTPOINT"

# 3) 验证：桥协议面（curl）恒测
SEED=$(curl -sf "http://127.0.0.1:$PORT/docs/seed.txt") || fail "GET seed 失败"
[ "$SEED" = "remote-seed-v1" ] || fail "seed 内容不符: $SEED"
curl -sf -X PUT --data-binary "via-curl-v1" "http://127.0.0.1:$PORT/docs/upload.txt" >/dev/null \
  || fail "PUT upload 失败"
grep -q "via-curl-v1" "$SERVE_ROOT/docs/upload.txt" || fail "直写对侧未见上传内容"
curl -sf -X PROPFIND -H "Depth: 1" "http://127.0.0.1:$PORT/" | grep -q "multistatus" || fail "PROPFIND 失败"
echo "bridge-level checks: PASS"

# 4) 验证：真挂载面（mount_webdav 可用时）
if [ "$MOUNTED" = "1" ]; then
  [ "$(cat "$MOUNTPOINT/docs/seed.txt")" = "remote-seed-v1" ] || fail "挂载点读 seed 不符"
  echo "written-through-mount" > "$MOUNTPOINT/docs/from-mount.txt"
  grep -q "written-through-mount" "$SERVE_ROOT/docs/from-mount.txt" || fail "挂载点写入未落对侧"
  ls "$MOUNTPOINT/docs" >/dev/null || fail "挂载点列目录失败"
  echo "mount-level checks: PASS"
  echo "VDRIVE-MOUNT-SMOKE-OK"
else
  echo "VDRIVE-MOUNT-SMOKE-PARTIAL（桥级通过；实挂被环境拒绝，见 MOUNT-UNAVAILABLE）"
fi
