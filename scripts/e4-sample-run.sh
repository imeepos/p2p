#!/usr/bin/env bash
# E4 采样运行器：管理本机节点 A 生命周期并调度 scripts/e4-ping-sample.sh 三连采样。
# 端点默认双 bootstrap(43.240.223.138 / 121.196.193.177, u3400) + 双 relay(同主机 u3403)，
# 可用 E4_BOOTSTRAP_1/2、E4_RELAY_1/2 覆盖；远端角色 E4_SSH_B(102 内网机)/E4_SSH_T(公网 ECS)。
# 红线：禁止按进程名杀进程（本机多实验节点会互杀），停止只读 PID 文件后对单个 PID 精确 kill。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

NODE_BIN="target/release/p2p-cli"
SAMPLER="${E4_SAMPLER:-$SCRIPT_DIR/e4-ping-sample.sh}"
RUN_DIR="${E4_RUN_DIR:-/tmp/e4-sample-run}"
PID_FILE="$RUN_DIR/node-a.pid"
RUST_LOG_VALUE="${RUST_LOG:-info,p2p_swarm=info}"
BOOTSTRAP_1="${E4_BOOTSTRAP_1:-43.240.223.138/u3400}"
BOOTSTRAP_2="${E4_BOOTSTRAP_2:-121.196.193.177/u3400}"
RELAY_1="${E4_RELAY_1:-43.240.223.138/u3403}"
RELAY_2="${E4_RELAY_2:-121.196.193.177/u3403}"
SSH_B="${E4_SSH_B:-imeepos@192.168.0.102}"
SSH_T="${E4_SSH_T:-root@121.196.193.177}"

PEER_ID=""; RUNS=3; DRY_RUN=0; SELF_CHECK=0
NODE_ARGS=(); SAMPLER_ARGS=(); ASKPASS_DIR=""

usage(){ cat >&2 <<'U'
usage: scripts/e4-sample-run.sh --peer-id <ID> [--bin SAMPLER] [--runs N] [--dry-run] [--self-check]
  --bin   采样器路径，默认 scripts/e4-ping-sample.sh；节点 A 固定 target/release/p2p-cli
  env:    E4_BOOTSTRAP_1/2 E4_RELAY_1/2 E4_SSH_B E4_SSH_T E4_RUN_DIR SSH_PASSWORD RUST_LOG
U
}
die(){ echo "e4-sample-run: $1" >&2; exit 2; }
now_utc(){ date -u +"%Y-%m-%dT%H:%M:%SZ"; }
valid_addr(){ printf '%s' "$1" | grep -qE '^[0-9A-Za-z.:_-]+/(u|t)[0-9]+$'; }
valid_peer(){ printf '%s' "$1" | grep -qE '^[1-9A-HJ-NP-Za-km-z]{40,52}$'; }  # base58 PeerId

while [ $# -gt 0 ]; do case "$1" in
  --peer-id) PEER_ID="${2:-}"; shift 2;;
  --bin) SAMPLER="${2:-}"; shift 2;;
  --runs) RUNS="${2:-}"; shift 2;;
  --dry-run) DRY_RUN=1; shift;;
  --self-check) SELF_CHECK=1; shift;;
  *) usage; die "unknown arg: $1";;
esac; done

check_config(){
  [ -n "$PEER_ID" ] || die "--peer-id required"
  valid_peer "$PEER_ID" || die "peer id not base58(40-52): $PEER_ID"
  local a
  for a in "$BOOTSTRAP_1" "$BOOTSTRAP_2" "$RELAY_1" "$RELAY_2"; do
    valid_addr "$a" || die "bad endpoint addr: $a (expect ip/uPORT)"
  done
  printf '%s' "$RUNS" | grep -qE '^[1-9][0-9]*$' || die "--runs must be a positive integer: $RUNS"
}

build_node_args(){
  NODE_ARGS=(node --data "$RUN_DIR/node-a" --name a --no-mdns
    --bootstrap "$BOOTSTRAP_1" --bootstrap "$BOOTSTRAP_2"
    --relay "$RELAY_1" --relay "$RELAY_2")
}
build_sampler_args(){
  SAMPLER_ARGS=(--peer-id "$PEER_ID" --runs "$RUNS" --bin "$NODE_BIN"
    --bootstrap "$BOOTSTRAP_1" --bootstrap "$BOOTSTRAP_2"
    --relay "$RELAY_1" --relay "$RELAY_2" --out "$RUN_DIR/sample.tsv")
}

remote_plan(){  # 仅打印远端节点启动形态；dry-run 只走到这里，绝不连接
  printf '  ssh %s "%s node --data ~/e4/%s --name %s --no-mdns --bootstrap %s --bootstrap %s --relay %s --relay %s &"\n' \
    "$1" "$NODE_BIN" "$2" "$2" "$BOOTSTRAP_1" "$BOOTSTRAP_2" "$RELAY_1" "$RELAY_2"
}

print_plan(){
  echo "E4 plan @ $(now_utc)"
  echo "  env: RUST_LOG=$RUST_LOG_VALUE E4_RUN_DIR=$RUN_DIR runs=$RUNS peer=$PEER_ID"
  echo "  local A: RUST_LOG=$RUST_LOG_VALUE $NODE_BIN ${NODE_ARGS[*]}"
  remote_plan "$SSH_B" b
  remote_plan "$SSH_T" t
  echo "  sampler: $SAMPLER ${SAMPLER_ARGS[*]}"
  echo "  pid policy: start_local 写 $PID_FILE；stop_local 只读该文件后对单个 PID 精确 kill，绝不按进程名杀"
}

dry_run(){
  check_config
  [ -x "$SAMPLER" ] || die "sampler not executable: $SAMPLER"
  build_node_args; build_sampler_args
  print_plan
  echo "DRY-RUN: 不连接远端、不启动节点、不执行采样器"
  echo "DRY-RUN OK"
}

start_local(){
  mkdir -p "$RUN_DIR"
  if [ -f "$PID_FILE" ]; then
    local old; old="$(cat "$PID_FILE")"
    kill -0 "$old" 2>/dev/null && die "node A already running, pid $old ($PID_FILE)"
    rm -f "$PID_FILE"  # 陈旧 PID 文件：清掉重写
  fi
  RUST_LOG="$RUST_LOG_VALUE" nohup "$NODE_BIN" "${NODE_ARGS[@]}" \
    >"$RUN_DIR/node-a.log" 2>&1 &
  echo "$!" >"$PID_FILE"
  wait_ready
}

wait_ready(){  # 日志出现 peer_id= 即视为就绪；进程早退立即报错
  local i ready=0
  for i in $(seq 1 30); do
    kill -0 "$(cat "$PID_FILE")" 2>/dev/null || die "node A exited early, see $RUN_DIR/node-a.log"
    if grep -q 'peer_id=' "$RUN_DIR/node-a.log" 2>/dev/null; then ready=1; break; fi
    sleep 1
  done
  [ "$ready" = 1 ] || { stop_local; die "node A not ready in 30s, see $RUN_DIR/node-a.log"; }
  echo "[run] node A peer_id=$(grep -oE 'peer_id=[A-Za-z0-9]+' "$RUN_DIR/node-a.log" | head -1 | cut -d= -f2)"
}

stop_local(){  # 只信 PID 文件：读取 -> 校验数字 -> 对该 PID 精确 TERM/KILL -> 删文件
  if [ ! -f "$PID_FILE" ]; then echo "[run] stop_local: no pidfile, nothing to stop" >&2; return 0; fi
  local pid; pid="$(cat "$PID_FILE")"
  case "$pid" in
    ''|*[!0-9]*) rm -f "$PID_FILE"; echo "[run] stop_local: bad pid '$pid', pidfile removed" >&2; return 0;;
  esac
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || echo "[run] stop_local: TERM pid $pid failed" >&2
    wait "$pid" 2>/dev/null || true  # node 是本脚本直接子进程，wait 即回收
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
      echo "[run] stop_local: pid $pid ignored TERM, sent KILL" >&2
    fi
  else
    echo "[run] stop_local: pid $pid not alive (stale pidfile)" >&2
  fi
  rm -f "$PID_FILE"
}

askpass_setup(){  # 密码只从 SSH_PASSWORD 进 0600 临时文件，绝不回显终端/日志
  [ -n "${SSH_PASSWORD:-}" ] || die "SSH_PASSWORD not set (required for remote ssh)"
  ASKPASS_DIR="$(mktemp -d "${TMPDIR:-/tmp}/e4-askpass.XXXXXX")"
  printf '%s' "$SSH_PASSWORD" >"$ASKPASS_DIR/pw"
  { echo '#!/bin/sh'; echo "cat '$ASKPASS_DIR/pw'"; } >"$ASKPASS_DIR/askpass.sh"
  chmod 600 "$ASKPASS_DIR/pw"; chmod 700 "$ASKPASS_DIR/askpass.sh"
}
askpass_teardown(){ if [ -n "$ASKPASS_DIR" ]; then rm -rf "$ASKPASS_DIR"; ASKPASS_DIR=""; fi; }
ssh_run(){  # $@ = user@host 与远端命令；askpass 注入，密码不进命令行
  SSH_ASKPASS="$ASKPASS_DIR/askpass.sh" SSH_ASKPASS_REQUIRE=force DISPLAY=:0 \
    command ssh -o StrictHostKeyChecking=accept-new "$@"
}
remote_start(){  # 真实模式且 E4_REMOTE_START=1 才真正连接；dry-run 路径不经过这里
  [ "${E4_REMOTE_START:-0}" = 1 ] || return 0
  [ -n "$ASKPASS_DIR" ] || askpass_setup
  ssh_run "$1" "nohup $NODE_BIN node --data \$HOME/e4/$2 --name $2 --no-mdns \
    --bootstrap '$BOOTSTRAP_1' --bootstrap '$BOOTSTRAP_2' --relay '$RELAY_1' --relay '$RELAY_2' \
    >\$HOME/e4-$2.log 2>&1 & echo \$! >\$HOME/e4-$2.pid"
}

cleanup(){ askpass_teardown; stop_local; }

run_real(){
  check_config
  [ -x "$NODE_BIN" ] || die "node binary missing: $NODE_BIN (cargo build --release -p p2p-cli)"
  [ -x "$SAMPLER" ] || die "sampler not executable: $SAMPLER"
  build_node_args; build_sampler_args
  trap cleanup EXIT
  remote_start "$SSH_B" b
  remote_start "$SSH_T" t
  start_local
  echo "[run] sampler: $SAMPLER ${SAMPLER_ARGS[*]}"
  set +e; "$SAMPLER" "${SAMPLER_ARGS[@]}"; RC=$?; set -e
  echo "[run] sampler exit=$RC, tsv: $RUN_DIR/sample.tsv"
  exit "$RC"
}

self_check(){
  local a fake joined
  for a in "$BOOTSTRAP_1" "$BOOTSTRAP_2" "$RELAY_1" "$RELAY_2"; do
    valid_addr "$a" || { echo "SELF-CHECK FAIL: endpoint $a"; exit 1; }
  done
  if valid_addr "1.2.3.4/x9"; then echo "SELF-CHECK FAIL: bad addr accepted"; exit 1; fi
  fake="$(printf 'A%.0s' $(seq 1 52))"
  valid_peer "$fake" || { echo "SELF-CHECK FAIL: peer id validation"; exit 1; }
  if valid_peer "too-short"; then echo "SELF-CHECK FAIL: bad peer accepted"; exit 1; fi
  PEER_ID="$fake"; build_node_args; build_sampler_args
  joined="${NODE_ARGS[*]} ${SAMPLER_ARGS[*]}"
  for a in "$BOOTSTRAP_1" "$BOOTSTRAP_2" "$RELAY_1" "$RELAY_2"; do
    printf '%s' "$joined" | grep -qF -- "$a" || { echo "SELF-CHECK FAIL: $a missing from commands"; exit 1; }
  done
  printf '%s' "${SAMPLER_ARGS[*]}" | grep -qF -- "$fake" || { echo "SELF-CHECK FAIL: peer id missing"; exit 1; }
  if grep -nEq 'p[k]ill|kill[a]ll' "$0"; then echo "SELF-CHECK FAIL: process-name killer present"; exit 1; fi
  grep -qF 'kill "$pid"' "$0" || { echo "SELF-CHECK FAIL: exact-pid kill missing"; exit 1; }
  grep -qF 'node-a.pid' "$0" || { echo "SELF-CHECK FAIL: pidfile path missing"; exit 1; }
  grep -qF 'SSH_ASKPASS' "$0" || { echo "SELF-CHECK FAIL: askpass wiring missing"; exit 1; }
  echo "SELF-CHECK PASS"
}

[ "$SELF_CHECK" = 1 ] && { self_check; exit 0; }
[ "$DRY_RUN" = 1 ] && { dry_run; exit 0; }
run_real
