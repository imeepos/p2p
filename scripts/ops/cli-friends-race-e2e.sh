#!/usr/bin/env bash
# R1 friends 并发写 E2E：两 p2pctl 进程并发 friend add 各 5 笔，断言零静默丢失。
# 场景：预热 1 笔（生成 key.seed 与 chat/ 目录，规避并发首生成竞态）→
#   A/B 双流并发各 add 5 笔（定值互异 peer）→ 终态断言：
#   CLI 冷读 yrs 日志合并视图恰 2N+1 = 11 笔全量在（少一笔即红，Y1 起 CRDT
#   合并无需文件锁）、无重复、无孤儿（peer 全落在预期集内）、磁盘日志头行
#   合法且更新行恰 2N+1、两次独立冷读一致、双流无 panic。
# 幂等：临时数据目录隔离，trap 清理（造数不过夜）。末行 FRIENDS-RACE-OK。
set -eu
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="${RACE_CTL:-$ROOT/apps/cli/target/debug/p2pctl}"
N=5
DD=""
A_LOG="" B_LOG=""

fail() { echo "FRIENDS-RACE FAIL: $1" >&2; exit 1; }

cleanup() { [ -n "$DD" ] && rm -rf "$DD"; return 0; }
trap cleanup EXIT

# 32 字节定值 peer id（base58），定值可断言成员且不与本机随机身份碰撞。
peer_id() {
  python3 -c '
import sys
A = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
n = int.from_bytes(bytes([int(sys.argv[1])]) * 32, "big")
s = ""
while n:
    n, r = divmod(n, 58)
    s = A[r] + s
print(s)
' "$1"
}

# 收集一个流的后台进程输出；panic 即失败。
assert_no_panic() {
  local log=$1 tag=$2
  grep -qi "panic" "$log" || return 0
  echo "--- $tag 流输出 ---" >&2; cat "$log" >&2
  fail "$tag 流出现 panic"
}

# 单流顺序 friend add N 笔（流内严格串行；跨流并发由后台任务保证）。
stream_add() {
  local tag=$1 data=$2 log=$3; shift 3
  local i=1 peer
  for peer in "$@"; do
    "$CTL" chat friends add "$peer" --nickname "race-$tag-$i" --json \
      --data-dir "$data" >>"$log" 2>&1 || fail "friend add 失败（流 $tag 第 $i 笔）"
    i=$((i + 1))
  done
}

# CLI 冷读磁盘 yrs 日志合并视图（每次调用均为新进程直读磁盘）。
collect_friends_view() {
  "$CTL" chat friends list --json --data-dir "$DD" > "$DD/.view.json" \
    || fail "friends list 读回失败"
}

# 终态：合并视图恰 2N+1 笔全量在簿、无重复、无孤儿；日志头行合法、更新行恰 2N+1。
assert_friends_full() {
  python3 - "$DD/.view.json" "$DD/chat/friends.json" "$((2 * N + 1))" \
    "$W_ID" "${A_IDS[@]}" "${B_IDS[@]}" <<'PY'
import base64, json, sys
view_path, log_path, want = sys.argv[1], sys.argv[2], int(sys.argv[3])
expected = set(sys.argv[4:])
book = json.load(open(view_path))
ids = [f["peerId"] for f in book]
assert len(ids) == len(set(ids)), f"好友 peer 重复: {ids}"
assert len(ids) == want, f"好友条数 {len(ids)} ≠ 全量 {want}：并发写发生静默丢失"
orphans = [i for i in ids if i not in expected]
assert not orphans, f"孤儿好友记录: {orphans}"
lines = open(log_path).read().splitlines()
header = json.loads(lines[0])
assert header.get("p2p-friends") == "yrs-v1", f"日志头行异常: {lines[0]}"
updates = [json.loads(l) for l in lines[1:] if l.strip()]
for u in updates:
    base64.b64decode(u["u"], validate=True)
assert len(updates) == want, f"更新行数 {len(updates)} ≠ 变更数 {want}：追加丢失或损坏"
print(f"yrs 日志合并视图全量 {want} 笔在簿，更新行 {len(updates)} 行级完整，零丢失零重复零孤儿")
PY
}

# 两次独立冷读一致：yrs 日志解码确定性，磁盘权威态稳定可读。
assert_friends_view_stable() {
  "$CTL" chat friends list --json --data-dir "$DD" > "$DD/.view2.json" \
    || fail "friends list 二次读回失败"
  python3 - "$DD/.view.json" "$DD/.view2.json" <<'PY'
import json, sys
a = json.load(open(sys.argv[1]))
b = json.load(open(sys.argv[2]))
assert a == b, "两次独立冷读不一致（解码非确定）"
print(f"CLI 两次独立冷读一致（{len(a)} 条）")
PY
}

# ---- 主流程 ----
DD="$(mktemp -d "${TMPDIR:-/tmp}/friends-race.XXXXXX")"
[ -x "$CTL" ] || fail "p2pctl 不存在: $CTL（先 cargo build --manifest-path apps/cli/Cargo.toml）"

echo "== 0. 预热（单次 friend add 生成 key.seed 与 chat/ 目录） =="
W_ID=$(peer_id 9)
"$CTL" chat friends add "$W_ID" --nickname race-warmup --json --data-dir "$DD" \
  >/dev/null || fail "预热 friend add 失败"
echo "预热完成 key.seed=$DD/key.seed"

A_IDS=() B_IDS=()
for i in 1 2 3 4 5; do A_IDS+=("$(peer_id "$i")"); B_IDS+=("$(peer_id "$((i + 10))")"); done

echo "== 1. 双流并发 friends add 各 $N 笔（yrs CRDT 合并，无文件锁） =="
A_LOG="$DD/a.log"; B_LOG="$DD/b.log"
stream_add A "$DD" "$A_LOG" "${A_IDS[@]}" & PA=$!
stream_add B "$DD" "$B_LOG" "${B_IDS[@]}" & PB=$!
wait "$PA" || fail "A 流 friends add 中止"
wait "$PB" || fail "B 流 friends add 中止"
assert_no_panic "$A_LOG" A; assert_no_panic "$B_LOG" B

echo "== 2. 终态断言：2N+1 全量在簿（零静默丢失） =="
collect_friends_view
assert_friends_full
assert_friends_view_stable
rm -f "$DD/.view.json" "$DD/.view2.json"
echo "并发语义实测：好友簿 yrs CRDT 合并（无锁），10 并发笔全量落盘"
echo "FRIENDS-RACE-OK"
