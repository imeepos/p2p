#!/usr/bin/env bash
# E5 长稳浸泡编排器：真实多机 + 双公网拓扑，24h 采集资源与行为数据。
# 拓扑：A=本机(macA) 客户端节点；B=102(LAN) 客户端；T=ECS(公网) 客户端；
#       bootstrap=138(systemd ops@) + ECS(systemd root@)，E5 起两台均仅密钥登录。
# 探测：每 SAMPLE_GAP 对 T 做 3 连 ping 采样；每 METRIC_GAP 采 A/B/T 的 RSS 与
#       指标快照行数；PERTURB_AT 小时处重启 ECS bootstrap 验证重连退避。
# 红线：远端一律精确 PID/单位文件操作，禁止按进程名杀进程；凭据不进命令行。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

NODE_BIN="${E5_NODE_BIN:-target/release/p2p-cli}"
NODE_BIN_B="${E5_NODE_BIN_B:-$NODE_BIN}"   # 102 远端二进制绝对路径
NODE_BIN_T="${E5_NODE_BIN_T:-$NODE_BIN}"   # ECS 远端二进制绝对路径
RUN_DIR="${E5_RUN_DIR:-/tmp/e5-soak}"
DURATION_H="${E5_DURATION_H:-24}"
SAMPLE_GAP_MIN="${E5_SAMPLE_GAP_MIN:-30}"
METRIC_GAP_MIN="${E5_METRIC_GAP_MIN:-5}"
PERTURB_AT_H="${E5_PERTURB_AT_H:-12}"
SSH_B="${E5_SSH_B:-imeepos@192.168.0.102}"
SSH_T="${E5_SSH_T:-root@121.196.193.177}"
BOOTSTRAP_1="${E5_BOOTSTRAP_1:-43.240.223.138/u3400}"
BOOTSTRAP_2="${E5_BOOTSTRAP_2:-121.196.193.177/u3400}"
RELAY_1="${E5_RELAY_1:-43.240.223.138/u3403}"
RELAY_2="${E5_RELAY_2:-121.196.193.177/u3403}"
SAMPLER="${E5_SAMPLER:-$SCRIPT_DIR/e4-ping-sample.sh}"

log(){ printf '[soak %s] %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }
valid_ssh(){ printf '%s' "$1" | grep -qE '^[A-Za-z0-9._-]+@[A-Za-z0-9.:_-]+$'; }
ssh_run(){ ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=15 "$@"; }

remote_start(){  # $1=user@host $2=name $3=remote bin；返回远端节点 pid
  local bin="$3"
  ssh_run "$1" "mkdir -p ~/e5 && \
    if [ -f ~/e5/$2.pid ] && kill -0 \$(cat ~/e5/$2.pid) 2>/dev/null; then echo ALREADY; exit 0; fi; \
    RUST_LOG=info P2P_METRICS_LOG_SECS=60 \
    nohup $bin node --data ~/e5/$2 --name $2 --no-mdns \
      --bootstrap '$BOOTSTRAP_1' --bootstrap '$BOOTSTRAP_2' \
      --relay '$RELAY_1' --relay '$RELAY_2' \
      >>~/e5/$2.log 2>&1 & echo \$! >~/e5/$2.pid; cat ~/e5/$2.pid"
}

remote_stop(){  # $1=user@host $2=name $3=remote bin；按 pid 文件+进程名双核对
  ssh_run "$1" "if [ -f ~/e5/$2.pid ]; then p=\$(cat ~/e5/$2.pid); \
    c=\$(ps -p \$p -o comm= 2>/dev/null | tail -1); \
    if [ \"\$(basename \"\${c:-}\")\" = \"\$(basename "$3")\" ]; then kill \$p; fi; rm -f ~/e5/$2.pid; fi"
}

remote_rss_kb(){  # $1=user@host $2=name
  ssh_run "$1" "p=\$(cat ~/e5/$2.pid 2>/dev/null || echo 0); \
    if [ \"\$p\" != 0 ] && ps -p \$p >/dev/null 2>&1; then ps -p \$p -o rss= | tr -d ' '; else echo DOWN; fi" 2>/dev/null || echo SSH_FAIL
}

remote_peer_id(){  # $1=user@host $2=name
  ssh_run "$1" "grep -oE 'peer_id=[A-Za-z0-9]+' ~/e5/$2.log 2>/dev/null | head -1 | cut -d= -f2" 2>/dev/null || true
}

local_rss_kb(){
  local pid; pid="$(cat "$RUN_DIR/node-a.pid" 2>/dev/null || echo 0)"
  [ "$pid" != 0 ] && ps -p "$pid" -o rss= 2>/dev/null | tr -d ' ' || echo DOWN
}

metric_lines(){  # 指标快照行累计数（A 本地日志）
  grep -c "metrics snapshot" "$RUN_DIR/node-a.log" 2>/dev/null || echo 0
}

sample_once(){  # 一轮资源+行为采样，追加 TSV
  local phase="$1"
  printf "%s\t%s\t%s\t%s\t%s\t%s\n" "$(date -u +%FT%TZ)" "$phase" \
    "$(local_rss_kb)" "$(remote_rss_kb "$SSH_B" b)" "$(remote_rss_kb "$SSH_T" t)" \
    "$(metric_lines)" >>"$RUN_DIR/resources.tsv"
}

run_probe(){  # 3 连采样并追加结果；失败不中断长稳，记 FAIL
  local out="$RUN_DIR/probe-$(date -u +%Y%m%dT%H%M%SZ).tsv"
  set +e
  RUST_LOG=info "$SAMPLER" --peer-id "$T_PEER_ID" --runs 3 --bin "$NODE_BIN" \
    --bootstrap "$BOOTSTRAP_1" --bootstrap "$BOOTSTRAP_2" \
    --relay "$RELAY_1" --relay "$RELAY_2" --out "$out" >>"$RUN_DIR/probe.log" 2>&1
  local rc=$?
  set -e
  tail -1 "$out" 2>/dev/null | sed "s/^/$(date -u +%FT%TZ)\t/" >>"$RUN_DIR/probes.tsv" \
    || printf "%s\tSAMPLE\tFAIL|no output (rc=%s)\n" "$(date -u +%FT%TZ)" "$rc" >>"$RUN_DIR/probes.tsv"
  log "probe rc=$rc -> $out"
}

perturb(){  # 预定扰动：重启 ECS bootstrap，观察 B/T/A 的重连与退避行为
  log "PERTURB start: restart ECS bootstrap (systemctl)"
  ssh_run "$SSH_T" "systemctl restart p2p-bootstrap" || log "PERTURB: restart cmd failed"
  log "PERTURB done; reconnection behavior expected in logs"
}

cleanup(){
  remote_stop "$SSH_B" b "$NODE_BIN_B" || true
  remote_stop "$SSH_T" t "$NODE_BIN_T" || true
  if [ -f "$RUN_DIR/node-a.pid" ]; then
    p="$(cat "$RUN_DIR/node-a.pid")"
    c="$(ps -p "$p" -o comm= 2>/dev/null | tail -1)"
    if [ "$(basename "${c:-}")" = "$(basename "$NODE_BIN")" ]; then kill "$p" || true; fi
    rm -f "$RUN_DIR/node-a.pid"
  fi
  log "cleanup done; data in $RUN_DIR"
}

main(){
  [ "$(id -u)" = 0 ] && die="soak must run as normal user" && echo "$die" >&2 && exit 2
  mkdir -p "$RUN_DIR"; chmod 700 "$RUN_DIR"; umask 077
  : >"$RUN_DIR/resources.tsv"; : >"$RUN_DIR/probes.tsv"
  printf "utc\tphase\tA_rss_kb\tB_rss_kb\tT_rss_kb\tA_metric_lines\n" >"$RUN_DIR/resources.tsv"
  printf "utc\tsample\n" >"$RUN_DIR/probes.tsv"

  log "starting remote nodes (B on $SSH_B, T on $SSH_T)"
  remote_start "$SSH_B" b "$NODE_BIN_B"
  remote_start "$SSH_T" t "$NODE_BIN_T"
  T_PEER_ID=""
  for i in $(seq 1 30); do
    T_PEER_ID="$(remote_peer_id "$SSH_T" t)"
    [ -n "$T_PEER_ID" ] && break
    sleep 2
  done
  [ -n "$T_PEER_ID" ] || { log "FATAL: T peer_id not found"; exit 1; }
  log "T peer_id=$T_PEER_ID"

  log "starting local node A"
  RUST_LOG="info,p2p_swarm=info" P2P_METRICS_LOG_SECS=60 \
    nohup "$NODE_BIN" node --data "$RUN_DIR/node-a" --name a --no-mdns \
      --bootstrap "$BOOTSTRAP_1" --bootstrap "$BOOTSTRAP_2" \
      --relay "$RELAY_1" --relay "$RELAY_2" \
      >>"$RUN_DIR/node-a.log" 2>&1 &
  echo $! >"$RUN_DIR/node-a.pid"
  sleep 5
  grep -q 'peer_id=' "$RUN_DIR/node-a.log" || { log "FATAL: A not ready"; cleanup; exit 1; }
  log "node A ready"

  trap cleanup EXIT
  local start; start=$(date +%s)
  local total_s=$((DURATION_H * 3600))
  local sample_gap_s=$((SAMPLE_GAP_MIN * 60))
  local metric_gap_s=$((METRIC_GAP_MIN * 60))
  local perturb_s=$((PERTURB_AT_H * 3600))
  local next_probe=$((start + 60))
  local next_metric=$start
  local perturb_at=$((start + perturb_s))
  local now
  log "soak for ${DURATION_H}h; probe every ${SAMPLE_GAP_MIN}min; metrics every ${METRIC_GAP_MIN}min; perturb at +${PERTURB_AT_H}h"
  while [ "$(date +%s)" -lt $((start + total_s)) ]; do
    now=$(date +%s)
    if [ "$PERTURB_AT_H" -gt 0 ] && [ "$now" -ge "$perturb_at" ]; then
      perturb; perturb_at=$((perturb_at + 999999999))  # 单次扰动
    fi
    if [ "$now" -ge "$next_metric" ]; then sample_once running; next_metric=$((now+metric_gap_s)); fi
    if [ "$now" -ge "$next_probe" ]; then run_probe; next_probe=$((now+sample_gap_s)); fi
    sleep 30
  done
  sample_once final
  log "soak complete: ${DURATION_H}h"
  log "report inputs: $RUN_DIR/resources.tsv $RUN_DIR/probes.tsv $RUN_DIR/node-a.log"
}

main "$@"
