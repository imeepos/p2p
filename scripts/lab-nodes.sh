#!/usr/bin/env bash
# 本机实验节点收编（E5 治理配套）：按实例名管理常驻 p2p 节点进程。
#
# 背景：实验节点曾以裸命令随处启动（/tmp data 目录、随端口漂移、退出无痕迹），
# 向公共 rendezvous 泄漏 loopback 注册且无人认领。本脚本用 PID 文件收编：
# 精确到进程终结，绝不 pkill 模式匹配（红线：.15 上 pkill 会互杀实验节点）。
#
# 用法：
#   scripts/lab-nodes.sh start <名字> -- <node 启动命令...>   # nohup 后台拉起
#   scripts/lab-nodes.sh stop <名字>                          # TERM→等 5s→KILL
#   scripts/lab-nodes.sh status                               # 全部实例一览
#   scripts/lab-nodes.sh stop-all
#
# 实例登记（名字 → 启动命令）建议集中写在 ~/p2p-lab/nodes.env：
#   coordinator=/path/p2p-cli node --data ~/p2p-lab/data/coordinator \
#     --name coordinator --bootstrap 43.240.223.138/u3400 \
#     --observation 43.240.223.138:3402
# 然后用 eval 包装：start coordinator -- $COORDINATOR_CMD
set -euo pipefail

RUN_DIR="${LAB_NODES_RUN_DIR:-$HOME/p2p-lab/run}"
LOG_DIR="${LAB_NODES_LOG_DIR:-$HOME/p2p-lab/logs}"
STOP_WAIT_SECS=5

mkdir -p "$RUN_DIR" "$LOG_DIR"

pid_file() { printf '%s/%s.pid' "$RUN_DIR" "$1"; }

is_running() {
  local pid
  pid="$(cat "$(pid_file "$1")" 2>/dev/null || true)"
  [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

cmd_start() {
  local name="$1"
  shift
  [ "${1:-}" = "--" ] && shift
  if [ "$#" -eq 0 ]; then
    echo "lab-nodes: start $name 缺少启动命令（start <名字> -- <命令...>）" >&2
    exit 2
  fi
  if is_running "$name"; then
    echo "lab-nodes: $name 已在运行 pid=$(cat "$(pid_file "$name")")"
    return 0
  fi
  local log="$LOG_DIR/$name.log"
  nohup "$@" >>"$log" 2>&1 &
  local pid=$!
  echo "$pid" > "$(pid_file "$name")"
  sleep 0.3
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "lab-nodes: $name 启动即退出，见 $log" >&2
    rm -f "$(pid_file "$name")"
    exit 1
  fi
  echo "lab-nodes: $name 已启动 pid=$pid log=$log"
}

cmd_stop() {
  local name="$1"
  if ! is_running "$name"; then
    echo "lab-nodes: $name 未在运行"
    rm -f "$(pid_file "$name")"
    return 0
  fi
  local pid
  pid="$(cat "$(pid_file "$name")")"
  kill "$pid"
  for _ in $(seq 1 "$STOP_WAIT_SECS"); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$pid" 2>/dev/null; then
    echo "lab-nodes: $name TERM 未退出，升级 KILL pid=$pid" >&2
    kill -9 "$pid"
  fi
  rm -f "$(pid_file "$name")"
  echo "lab-nodes: $name 已停止"
}

cmd_status() {
  local found=0
  for f in "$RUN_DIR"/*.pid; do
    [ -e "$f" ] || continue
    found=1
    local name pid state
    name="$(basename "$f" .pid)"
    pid="$(cat "$f")"
    if kill -0 "$pid" 2>/dev/null; then state=running; else state=dead; fi
    printf '%-16s %-8s pid=%s\n' "$name" "$state" "$pid"
  done
  [ "$found" -eq 1 ] || echo "（无登记实例；登记见脚本头注释）"
}

case "${1:-}" in
  start) shift; cmd_start "$@" ;;
  stop) shift; cmd_stop "${1:-}" ;;
  stop-all)
    for f in "$RUN_DIR"/*.pid; do
      [ -e "$f" ] || continue
      cmd_stop "$(basename "$f" .pid)"
    done
    ;;
  status) cmd_status ;;
  *)
    grep '^# 用法' -A 6 "$0" | sed 's/^# \{0,2\}//'
    exit 2
    ;;
esac
