#!/usr/bin/env bash
# vdrive 跨机真挂载冒烟：102(Linux x86_64) 跑 serve，Mac 挂载实测。
# 口径 VDRIVE-LAN-SMOKE-OK：经真实网络（非 loopback）完成挂载点双向读写。
# 依赖：ssh 免密到 ${HOST}；本仓库 main 可构建（cargo 1.9x）。
set -euo pipefail

HOST="${VDRIVE_REMOTE:-imeepos@192.168.0.102}"
REMOTE_DIR="${VDRIVE_REMOTE_DIR:-\$HOME/vdrive-lan-smoke}"
LAN_IP="${VDRIVE_REMOTE_IP:-192.168.0.102}"
QUIC_PORT="${VDRIVE_QUIC_PORT:-54000}"
NAME="vdrive-lan-$$"
MOUNT="$HOME/.vdrive-mounts/$NAME"
WORK="$(mktemp -d /tmp/vdrive-lan.XXXXXX)"
PIDS=()
MOUNTED=0
MOUNTPOINT=""

cleanup() {
  if [ "$MOUNTED" = "1" ]; then diskutil unmount force "$MOUNTPOINT" >/dev/null 2>&1 || true; fi
  for pid in "${PIDS[@]:-}"; do kill "$pid" >/dev/null 2>&1 || true; done
  ssh -o ConnectTimeout=5 "$HOST" "pkill -f 'vdrive serve --root /tmp/vdrive-lan-root' >/dev/null 2>&1 || true" >/dev/null 2>&1 || true
  wait 2>/dev/null || true
  rm -rf "$WORK" "$HOME/.vdrive-mounts/$NAME"
  rmdir "$HOME/.vdrive-mounts" 2>/dev/null || true
}
trap cleanup EXIT

fail() { echo "vdrive-lan-smoke: FAIL: $*" >&2; exit 1; }
ready_field() { python3 -c "import json,sys; d=json.loads(sys.stdin.read() or '{}'); print(d.get('$1',''))"; }

ssh -o ConnectTimeout=5 "$HOST" 'echo probe-ok' >/dev/null || fail "ssh 不可达: $HOST"

# 1) 仓库以 bundle 投递（self-contained，不依赖 102 访问 github）
echo "[1/4] 投递仓库 bundle …"
git -C "$(pwd)" bundle create "$WORK/p2p.bundle" main >/dev/null 2>&1 || fail "bundle 创建失败"
scp -q "$WORK/p2p.bundle" "$HOST:/tmp/p2p-lan-smoke.bundle"
ssh "$HOST" "rm -rf $REMOTE_DIR /tmp/p2p-lan-smoke.bundle.tmp && \
  git clone -q /tmp/p2p-lan-smoke.bundle $REMOTE_DIR -b main && \
  cd $REMOTE_DIR && git bundle verify /tmp/p2p-lan-smoke.bundle >/dev/null 2>&1 || true"

# 2) 102 构建并启动 serve（marker 便于收口 pkill）
echo "[2/4] 102 构建 p2pctl（首次数分钟）…"
ssh "$HOST" "cd $REMOTE_DIR/apps/cli && cargo build -q 2>&1 | tail -2; [ -x target/debug/p2pctl ] || exit 1"
ssh "$HOST" "rm -rf /tmp/vdrive-lan-root && mkdir -p /tmp/vdrive-lan-root/docs && \
  echo remote-102-seed > /tmp/vdrive-lan-root/docs/seed.txt && \
  cd $REMOTE_DIR/apps/cli && nohup ./target/debug/p2pctl vdrive serve \
    --root /tmp/vdrive-lan-root --data-dir /tmp/vdrive-lan-id --no-mdns \
    --quic-port $QUIC_PORT >/tmp/vdrive-lan-serve.out 2>/tmp/vdrive-lan-serve.err & echo \$!"
sleep 1
for _ in $(seq 1 60); do
  ssh "$HOST" "grep -q '\"kind\":\"ready\"' /tmp/vdrive-lan-serve.out 2>/dev/null" && break
  sleep 1
done
ssh "$HOST" "grep -q '\"kind\":\"ready\"' /tmp/vdrive-lan-serve.out" || {
  ssh "$HOST" "tail -5 /tmp/vdrive-lan-serve.err" >&2; fail "102 serve 未就绪"; }
READY_LINE=$(ssh "$HOST" "grep ready /tmp/vdrive-lan-serve.out | tail -1")
PEER=$(printf '%s' "$READY_LINE" | ready_field peerId)
# 监听串里的 127.0.0.1 替换为对端 LAN IP（跨机拨号必须非环回）
ADDR=$(printf '%s' "$READY_LINE" | python3 -c "
import json,sys
d=json.loads(sys.stdin.read() or '{}')
for a in d.get('listenAddrs',[]):
    ip,port=a.rsplit('/',1)[0],a.rsplit('/',1)[1]
    if not ip.startswith('127.'):
        print(f'{ip}/{port}'); break
else:
    print(f'$LAN_IP/{d.get(\"listenAddrs\",[\"\"])[0].rsplit(\"/\",1)[1] if d.get(\"listenAddrs\") else str($QUIC_PORT)}')")
[ -n "$PEER" ] || fail "缺 peerId"
echo "102 serve ready: peer=$PEER addr=$ADDR"

# 3) Mac 挂载（真实网络路径）
echo "[3/4] 本机挂载 102 目录 …"
"$PWD/apps/cli/target/debug/p2pctl" vdrive mount --peer "$PEER" --addr "$ADDR" \
  --data-dir "$WORK/id" --no-mdns --mount --name "$NAME" \
  > "$WORK/mount.out" 2> "$WORK/mount.err" &
PIDS+=($!)
for _ in $(seq 1 60); do
  grep -q '"kind":"ready"' "$WORK/mount.out" 2>/dev/null && break
  kill -0 "${PIDS[0]}" 2>/dev/null || { cat "$WORK/mount.err" >&2; fail "mount 进程早退"; }
  sleep 1
done
grep -q '"kind":"ready"' "$WORK/mount.out" || { cat "$WORK/mount.err" >&2; fail "mount 未就绪"; }
MOUNTPOINT=$(grep '"kind":"mounted"' "$WORK/mount.out" | tail -n 1 | ready_field mountpoint)
[ -n "$MOUNTPOINT" ] && mount | grep -q "$MOUNTPOINT" && MOUNTED=1
echo "mount ready: mountpoint=$MOUNTPOINT mounted=$MOUNTED"

# 4) 验证：跨机双向读写
echo "[4/4] 跨机读写验证 …"
[ "$MOUNTED" = "1" ] || fail "未完成挂载（桥级可查 $WORK/mount.out）"
[ "$(cat "$MOUNTPOINT/docs/seed.txt")" = "remote-102-seed" ] || fail "挂载点读 102 seed 不符"
echo "written-from-mac-over-lan" > "$MOUNTPOINT/docs/from-mac.txt"
ssh "$HOST" "grep -q written-from-mac-over-lan /tmp/vdrive-lan-root/docs/from-mac.txt" \
  || fail "Mac 写入未落 102 磁盘"
echo "written-on-102" | ssh "$HOST" "cat > /tmp/vdrive-lan-root/docs/from-102.txt"
[ "$(cat "$MOUNTPOINT/docs/from-102.txt")" = "written-on-102" ] || fail "102 写入未见于挂载点"
echo "LAN checks: PASS"
echo "VDRIVE-LAN-SMOKE-OK"
