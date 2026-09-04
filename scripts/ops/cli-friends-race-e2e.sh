#!/usr/bin/env bash
# R1 friends 并发写 E2E：两 p2pctl 进程并发 friend add 各 5 笔，断言零静默丢失。
# 场景：预热 1 笔（生成 key.seed 与 chat/ 目录，规避并发首生成竞态）→
#   A/B 双流并发各 add 5 笔（定值互异 peer）→ 终态断言：
#   friends.json 合法 JSON、恰 2N+1 = 11 笔全量在（少一笔即红）、
#   无重复、无孤儿（peer 全落在预期集内）、CLI 读回 == 磁盘、双流无 panic。
# 丢写即红对应红线：并发写不允许无提示丢失；锁僵持走显式超时报错
# （「拒绝静默覆盖」），冲突路径可观测而非静默。
# 幂等：临时数据目录隔离，trap 清理（造数不过夜）。末行 FRIENDS-RACE-OK。
set -euo pipefail

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

# friends.json 终态：恰 2N+1 笔全量在簿、无重复、无孤儿。
assert_friends_full() {
  python3 - "$DD/chat/friends.json" "$((2 * N + 1))" \
    "$W_ID" "${A_IDS[@]}" "${B_IDS[@]}" <<'PY'
import json, sys
path, want = sys.argv[1], int(sys.argv[2])
expected = set(sys.argv[3:])
with open(path) as f:
    book = json.load(f)
assert isinstance(book, list), "friends.json 不是 JSON 数组"
ids = [f["peerId"] for f in book]
assert len(ids) == len(set(ids)), f"好友 peer 重复: {ids}"
assert len(ids) == want, f"好友条数 {len(ids)} ≠ 全量 {want}：并发写发生静默丢失"
orphans = [i for i in ids if i not in expected]
assert not orphans, f"孤儿好友记录: {orphans}"
print(f"friends.json 全量 {want} 笔在簿，零丢失零重复零孤儿")
PY
}

# CLI 读回 == 磁盘文件（同一份好友簿两种读路径一致）。
assert_friends_view_matches_disk() {
  "$CTL" chat friends list --json --data-dir "$DD" > "$DD/.view.json" \
    || fail "friends list 读回失败"
  python3 - "$DD/.view.json" "$DD/chat/friends.json" <<'PY'
import json, sys
view = json.load(open(sys.argv[1]))
disk = json.load(open(sys.argv[2]))
assert view == disk, "CLI 好友簿读回与磁盘文件不一致"
print(f"CLI 好友簿读回 == 磁盘（{len(view)} 条）")
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

echo "== 1. 双流并发 friends add 各 $N 笔（跨进程文件锁串行合并） =="
A_LOG="$DD/a.log"; B_LOG="$DD/b.log"
stream_add A "$DD" "$A_LOG" "${A_IDS[@]}" & PA=$!
stream_add B "$DD" "$B_LOG" "${B_IDS[@]}" & PB=$!
wait "$PA" || fail "A 流 friends add 中止"
wait "$PB" || fail "B 流 friends add 中止"
assert_no_panic "$A_LOG" A; assert_no_panic "$B_LOG" B

echo "== 2. 终态断言：2N+1 全量在簿（零静默丢失） =="
assert_friends_full
assert_friends_view_matches_disk
rm -f "$DD/.view.json"
echo "并发语义实测：好友簿跨进程文件锁串行合并，10 并发笔全量落盘"
echo "FRIENDS-RACE-OK"
