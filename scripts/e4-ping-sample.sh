#!/usr/bin/env bash
# E4 三连 ping 采样：对同一目标连续 N 轮 ping，输出 TSV 结构化结果（可直接贴协调表）。
# 每轮：UTC / 轮次 / 命中路径 / 逐跳耗时与详情 / RTT / 失败原因；三轮全 OK 且路径一致才 PASS。
# 硬红线：本脚本族禁止按进程名杀进程（.15 本机会互杀实验节点），只允许精确 PID/单位文件。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "${BASH_SOURCE[0]}")/.."

BIN="${P2P_CLI_BIN:-target/release/p2p-cli}"
RUNS=3; WAIT=15; PEER_ID=""; OUT=""; RAW_LOG="${E4_RAW_LOG:-/tmp/e4-sample-raw.log}"
BOOTSTRAPS=(); RELAYS=()
RUST_LOG_VALUE="info"  # DialHop 逐跳已 info 化，采样默认日志 info 即可

usage(){ echo "usage: $0 --peer-id <ID> [--runs N] [--wait S] [--bin PATH] [--out FILE]" >&2;
  echo "       [--bootstrap ADDR]... [--relay ADDR]... [--dry-run] [--self-check]" >&2; }
die(){ echo "e4-ping-sample: $1" >&2; exit 2; }
now_ms(){ perl -MTime::HiRes=time -e 'printf("%d", time*1000)'; }
now_utc(){ date -u +"%Y-%m-%dT%H:%M:%SZ"; }
valid_addr(){ echo "$1" | grep -qE "^[0-9A-Za-z.:_-]+/(u|t)[0-9]+$"; }
valid_peer(){ echo "$1" | grep -qE "^[1-9A-HJ-NP-Za-km-z]{40,52}$"; }  # base58 PeerId

while [ $# -gt 0 ]; do case "$1" in
  --peer-id) PEER_ID="${2:-}"; shift 2;;
  --runs) RUNS="${2:-3}"; shift 2;;
  --wait) WAIT="${2:-15}"; shift 2;;
  --bin) BIN="${2:-}"; shift 2;;
  --out) OUT="${2:-}"; shift 2;;
  --bootstrap) BOOTSTRAPS+=("${2:-}"); shift 2;;
  --relay) RELAYS+=("${2:-}"); shift 2;;
  --dry-run) DRY_RUN=1; shift;;
  --self-check) SELF_CHECK=1; shift;;
  *) usage; die "unknown arg: $1";;
esac; done

check_args(){
  [ -n "$PEER_ID" ] || die "--peer-id required"
  valid_peer "$PEER_ID" || die "peer id not base58(40-52): $PEER_ID"
  [ "${#BOOTSTRAPS[@]}" -ge 1 ] || die "at least one --bootstrap required"
  local a; for a in "${BOOTSTRAPS[@]}"; do valid_addr "$a" || die "bad bootstrap addr: $a"; done
  for a in "${RELAYS[@]:-}"; do [ -z "$a" ] && continue; valid_addr "$a" || die "bad relay addr: $a"; done
  [ -x "$BIN" ] || die "binary not executable: $BIN (use --bin or P2P_CLI_BIN)"
}

# 流式给每行打毫秒时间戳（hop 行到达时刻即事件时刻，供逐跳耗时）
ts_lines(){ while IFS= read -r l; do printf "%s|%s\n" "$(now_ms)" "$l"; done; }

# 抽 hop/pong 字段（纯参数展开，避免 sed 转义地狱）
hop_of(){ printf "%s" "${1#hop }" | cut -d" " -f1; }
ok_of(){ local o="${1#* ok=}"; printf "%s" "${o%% *}"; }
detail_of(){ local d="${1#*detail=}"; printf "%s" "${d% (*}"; }
peer_of(){ local p="${1##*\(}"; printf "%s" "${p%\)}"; }

parse_round(){
  local first_ts="" last_ts="" path="" hops="" details="" rtt="" reason="" ts line row
  while IFS= read -r row; do
    ts="${row%%|*}"; line="${row#*|}"
    case "$line" in
      "hop "*)
        [ -z "$first_ts" ] && first_ts="$ts"
        if [ -n "$last_ts" ]; then hops="${hops:+$hops,}$(hop_of "$line"):$((ts-last_ts))ms"; fi
        last_ts="$ts"
        path="${path:+$path>}$(hop_of "$line")($(ok_of "$line"))"
        details="${details:+$details,}$(hop_of "$line"):$(detail_of "$line")"
        ;;
      "pong "*) rtt="${line#*rtt=}"; rtt="${rtt%% *}" ;;
      *"错误"*|*error*|*"请求失败"*) [ -z "$rtt" ] && reason="$(printf "%s" "$line" | cut -c1-120)" ;;
    esac
  done
  printf "%s\t%s\t%s\t%s\t%s\n" "${path:-none}" "${hops:-}" "${details:-}" "${rtt:-}" "${reason:--}"
}

build_args(){
  ARGS=(ping "$PEER_ID" --no-mdns --wait "$WAIT")
  local a; for a in "${BOOTSTRAPS[@]}"; do ARGS+=(--bootstrap "$a"); done
  for a in "${RELAYS[@]:-}"; do [ -z "$a" ] && continue; ARGS+=(--relay "$a"); done
}

run_once(){
  local n="$1" log; log="$(mktemp)"; build_args
  echo "[$(now_utc)] round $n/$RUNS -> $PEER_ID" >&2
  RUST_LOG="$RUST_LOG_VALUE" "$BIN" "${ARGS[@]}" 2>>"$log" | ts_lines > "$log.tsv" || true
  parse_round < "$log.tsv"
  { echo "----- round $n raw -----"; cat "$log.tsv"; echo "--- stderr tail ---"; tail -5 "$log"; } >>"$RAW_LOG"
  rm -f "$log" "$log.tsv"
}

emit_row(){  # 补时间戳与判定列，输出标准 TSV 行
  local n="$1" pr="$2" path hops det rtt reason verdict
  path="$(printf "%s" "$pr" | cut -f1)"; hops="$(printf "%s" "$pr" | cut -f2)"
  det="$(printf "%s" "$pr" | cut -f3)";  rtt="$(printf "%s" "$pr" | cut -f4)"
  reason="$(printf "%s" "$pr" | cut -f5)"
  verdict=FAIL; [ -n "$rtt" ] && verdict=OK
  printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n" "$(now_utc)" "$n" "$path" "$hops|$det" "${rtt:--}" "${reason:--}" "$verdict"
}

verdict_of(){
  local ok_count=0 path_first="" p v row
  while IFS="$TAB" read -r _ _ p _ _ _ v; do
    [ "$v" = OK ] && ok_count=$((ok_count+1))
    if [ -z "$path_first" ]; then path_first="$p";
    elif [ "$p" != "$path_first" ]; then echo "FAIL|path mismatch: $path_first vs $p"; return 0; fi
  done
  [ "$ok_count" -eq "$RUNS" ] && echo "PASS|all $RUNS rounds via $path_first" \
    || echo "FAIL|$ok_count/$RUNS rounds OK via $path_first"
}

self_check(){
  valid_addr "121.196.193.177/u3403" || { echo "SELF-CHECK FAIL: addr"; exit 1; }
  valid_addr "43.240.223.138/t3404" || { echo "SELF-CHECK FAIL: addr tcp"; exit 1; }
  if valid_addr "121.196.193.177/x3403"; then echo "SELF-CHECK FAIL: bad addr accepted"; exit 1; fi
  local fake_peer; fake_peer="$(printf "A%.0s" $(seq 1 52))"
  valid_peer "$fake_peer" || { echo "SELF-CHECK FAIL: peer"; exit 1; }
  if valid_peer "short"; then echo "SELF-CHECK FAIL: bad peer accepted"; exit 1; fi
  local hop_line="hop Direct ok=false detail=2 addr(s) tried ($fake_peer)"
  [ "$(hop_of "$hop_line")" = Direct ] && [ "$(ok_of "$hop_line")" = false ] \
    && [ "$(detail_of "$hop_line")" = "2 addr(s) tried" ] \
    && [ "$(peer_of "$hop_line")" = "$fake_peer" ] || { echo "SELF-CHECK FAIL: hop fields"; exit 1; }
  TAB=$'\t'
  local three; three="$(printf "u\t1\tDirect>Punch>Relay\t-\t-\t-\tOK\nu\t2\tDirect>Punch>Relay\t-\t-\t-\tOK\nu\t3\tDirect>Punch>Relay\t-\t-\t-\tOK\n")"
  # 命令替换剥尾换行：verdict_of 输入补尾换行，否则 while read 吞最后一行
  printf "%s\n" "$three" | { RUNS=3 verdict_of; } | grep -q "^PASS|" || { echo "SELF-CHECK FAIL: verdict"; exit 1; }
  local mix; mix="$(printf "u\t1\tDirect\t-\t-\t-\tOK\nu\t2\tRelay\t-\t-\t-\tOK\n")"
  printf "%s\n" "$mix" | { RUNS=2 verdict_of; } | grep -q "^FAIL|" || { echo "SELF-CHECK FAIL: mismatch not caught"; exit 1; }
  if grep -q "p[k]ill" "$0"; then echo "SELF-CHECK FAIL: process-name killer present"; exit 1; fi
  echo "SELF-CHECK PASS"
}

[ "${SELF_CHECK:-0}" = 1 ] && { self_check; exit 0; }
check_args

if [ "${DRY_RUN:-0}" = 1 ]; then
  build_args
  echo "DRY-RUN plan @ $(now_utc)"
  echo "  bin=$BIN runs=$RUNS wait=${WAIT}s rust_log=$RUST_LOG_VALUE"
  echo "  bootstrap: ${BOOTSTRAPS[*]}"
  echo "  relay: ${RELAYS[*]:-none}"
  echo "  would run: RUST_LOG=$RUST_LOG_VALUE $BIN ${ARGS[*]}"
  echo "DRY-RUN OK"
  exit 0
fi

TAB=$'\t'
NL=$'\n'   # 注意: $() 会剥尾换行, 换行符只能这样取
HDR="utc${TAB}round${TAB}path${TAB}per_hop${TAB}rtt${TAB}reason${TAB}verdict"
ROWS=""
for i in $(seq 1 "$RUNS"); do
  ROWS="${ROWS}$(emit_row "$i" "$(run_once "$i")")${NL}"
done
if [ -n "$OUT" ]; then
  { printf "%s\n" "$HDR"; printf "%s" "$ROWS"; } > "$OUT"
else
  { printf "%s\n" "$HDR"; printf "%s" "$ROWS"; }
fi
V="$(printf "%s" "$ROWS" | verdict_of)"
printf "SAMPLE\t%s\n" "$V"
[ -n "$OUT" ] && printf "SAMPLE\t%s\n" "$V" >> "$OUT"
case "$V" in PASS*) exit 0;; *) exit 1;; esac
