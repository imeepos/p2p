#!/usr/bin/env bash
# N2-R2 E2E：两个 p2pctl 进程流并发写同一数据目录的存储一致性。
# 场景与断言：
#   0) 预热单次 friend add：生成 key.seed 与 chat/ 目录（规避并发首生成竞态）；
#   1) 双流并发 chat friends add 各 N 笔（共享 friends.json yrs 更新日志，Y1 起）：
#      CLI 冷读合并视图条数恰为 2N+1（CRDT 合并、无需文件锁，零静默丢失）、
#      成员无孤儿（peer 全落在预期集内）、无重复、磁盘日志头行合法且更新行
#      恰 2N+1（一次变更一行，行级完整）、两次独立冷读一致、无 panic；
#   2) chat send 流各 N 笔（各自不可达 peer，消息以 pending 落盘）：D6 起同
#      数据目录身份单进程持有（try_lock_identity 被占即结构化拒绝），跨进程
#      并发 send 属被拒语义，故流间串行、并发拒绝另设确定性探针；每 peer
#      恰 N 条、id 唯一、内容与发送序一致、CLI history == 磁盘、messages/
#      目录无孤儿文件、未送达结构化信号（未送达）、无 panic。
# 幂等：临时数据目录隔离，trap 清理（造数不过夜）。末行 N2-R2-OK。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="${N2_CTL:-$ROOT/apps/cli/target/debug/p2pctl}"
N=5
DD=""
A_LOG="" B_LOG="" SA_LOG="" SB_LOG=""

fail() { echo "N2-R2 FAIL: $1" >&2; exit 1; }

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

# 收集一个流的后台进程输出；panic 即失败（结构化错误与预期信号另行断言）。
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
    "$CTL" chat friends add "$peer" --nickname "n2-$tag-$i" --json \
      --data-dir "$data" >>"$log" 2>&1 || fail "friend add 失败（流 $tag 第 $i 笔）"
    i=$((i + 1))
  done
}

# 单流顺序 chat send N 笔：对端不可达 → 结构化未送达（退出码 1 属预期）。
stream_send() {
  local tag=$1 data=$2 log=$3 target=$4
  local i=1 out rc
  while [ "$i" -le "$N" ]; do
    out=$("$CTL" chat send --peer "$target" --text "n2-$tag-$i" \
      --timeout-secs 1 --json --data-dir "$data" 2>&1) && rc=0 || rc=$?
    if [ "$rc" -ne 1 ] || ! printf "%s" "$out" | grep -q "未送达"; then
      printf "%s\n" "$out" >>"$log"
      fail "chat send 流 $tag 第 $i 笔未出现预期未送达信号（rc=$rc）"
    fi
    printf "%s\n" "$out" >>"$log"
    i=$((i + 1))
  done
}

# CLI 冷读磁盘 yrs 日志合并视图（每次调用均为新进程直读磁盘）。
collect_friends_view() {
  "$CTL" chat friends list --json --data-dir "$DD" > "$DD/.view.json" \
    || fail "friends list 读回失败"
}

# 终态：合并视图恰 2N+1 全量在簿（CRDT 合并零丢失）、无重复、无孤儿；
# 磁盘日志头行合法、更新行恰 2N+1（一次变更一行，行级完整）。
assert_friends_file() {
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
print(f"好友簿 yrs 日志合并视图全量 {want} 笔在簿，更新行 {len(updates)} 行级完整，无孤儿无重复")
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


# D6 确定性探针（独立临时目录）：首笔 send 持身份锁约 1s（未送达重投等待窗），
# 期内第二笔必须被结构化拒绝（身份被占用，rc=1）——同目录多程序并行不静默并存。
assert_d6_identity_busy_rejection() {
  local dd p1 out2 rc2
  dd="$(mktemp -d "${TMPDIR:-/tmp}/n2-r2-d6.XXXXXX")"
  "$CTL" chat friends add "$(peer_id 40)" --nickname d6-warmup --json --data-dir "$dd" \
    >/dev/null || fail "D6 探针预热失败"
  "$CTL" chat send --peer "$(peer_id 41)" --text d6-holder --timeout-secs 1 --json \
    --data-dir "$dd" > "$dd/first.out" 2>&1 & p1=$!
  sleep 0.3
  out2=$("$CTL" chat send --peer "$(peer_id 42)" --text d6-loser --timeout-secs 1 --json \
    --data-dir "$dd" 2>&1) && rc2=0 || rc2=$?
  wait "$p1" || true
  [ "$rc2" -ne 0 ] || fail "D6 探针：并发第二笔竟成功（身份锁未生效）"
  printf "%s" "$out2" | grep -q "身份被占用" || fail "D6 探针拒绝缺结构化信号: $out2"
  grep -q "未送达" "$dd/first.out" || fail "D6 探针：持锁首笔缺未送达信号"
  rm -rf "$dd"
  echo "D6 实测：同目录并发 send 被身份锁结构化拒绝（身份被占用），无静默并存"
}

# 每 peer 消息文件：恰 N 条、id 唯一、内容与发送序一致、状态 pending。
assert_peer_messages() {
  python3 - "$1" "$N" "$2" <<'PY'
import json, sys
path, n, tag = sys.argv[1], int(sys.argv[2]), sys.argv[3]
with open(path) as f:
    envs = [json.loads(line) for line in f if line.strip()]
assert len(envs) == n, f"{path}: 期望 {n} 条，实得 {len(envs)}"
ids = [e["id"] for e in envs]
assert len(set(ids)) == len(ids), f"{path}: 消息 id 重复"
texts = [e["text"] for e in envs]
want = {f"n2-{tag}-{i}" for i in range(1, n + 1)}
assert set(texts) == want, f"{path}: 消息内容不符: {sorted(texts)}"
assert all(e["status"] == "pending" for e in envs), f"{path}: 状态非 pending"
assert all(e["sender"] == "me" for e in envs), f"{path}: sender 非 me"
print(f"{tag}: {n} 条消息落盘完整，id 唯一，pending 可读")
PY
}

# CLI history 读回 == 磁盘行（同一消息库两种读路径一致）。
assert_history_view_matches_disk() {
  local peer=$1 file=$2
  "$CTL" chat history --peer "$peer" --limit 100 --json --data-dir "$DD" \
    > "$DD/.hist.json" || fail "chat history 读回失败（$peer）"
  python3 - "$DD/.hist.json" "$file" <<'PY'
import json, sys
view = json.load(open(sys.argv[1]))
disk_ids = [json.loads(l)["id"] for l in open(sys.argv[2]) if l.strip()]
view_ids = sorted(m["id"] for m in view)
assert view_ids == sorted(disk_ids), "CLI history 读回与磁盘消息不一致"
print(f"CLI history 读回 == 磁盘（{len(view)} 条）")
PY
}

# messages/ 目录文件集合 == 预期两 peer，无孤儿文件。
assert_messages_dir_clean() {
  python3 - "$DD/chat/messages" "$SA_ID" "$SB_ID" <<'PY'
import os, sys
d, sa, sb = sys.argv[1], sys.argv[2], sys.argv[3]
files = sorted(os.listdir(d))
assert files == sorted([sa + ".jsonl", sb + ".jsonl"]), f"messages/ 孤儿文件: {files}"
print("messages/ 目录无孤儿文件:", ", ".join(files))
PY
}

# ---- 主流程 ----
DD="$(mktemp -d "${TMPDIR:-/tmp}/n2-r2.XXXXXX")"
[ -x "$CTL" ] || fail "p2pctl 不存在: $CTL（先 cargo build --manifest-path apps/cli/Cargo.toml）"

echo "== 0. 预热（单次 friend add 生成 key.seed 与 chat/ 目录） =="
W_ID=$(peer_id 9)
"$CTL" chat friends add "$W_ID" --nickname n2-warmup --json --data-dir "$DD" \
  >/dev/null || fail "预热 friend add 失败"
echo "预热完成 key.seed=$DD/key.seed"

A_IDS=() B_IDS=()
for i in 1 2 3 4 5; do A_IDS+=("$(peer_id "$i")"); B_IDS+=("$(peer_id "$((i + 9))")"); done
SA_ID=$(peer_id 30); SB_ID=$(peer_id 31)

echo "== 1. 双流并发 friends add 各 $N 笔（共享 friends.json yrs 日志） =="
A_LOG="$DD/a.log"; B_LOG="$DD/b.log"
stream_add A "$DD" "$A_LOG" "${A_IDS[@]}" & PA=$!
stream_add B "$DD" "$B_LOG" "${B_IDS[@]}" & PB=$!
wait "$PA" || fail "A 流 friends add 中止"
wait "$PB" || fail "B 流 friends add 中止"
assert_no_panic "$A_LOG" A; assert_no_panic "$B_LOG" B
collect_friends_view
assert_friends_file
assert_friends_view_stable

echo "== 2. chat send 流各 $N 笔（pending 落盘；D6 单身份单进程，流间串行） =="
SA_LOG="$DD/sa.log"; SB_LOG="$DD/sb.log"
stream_send A "$DD" "$SA_LOG" "$SA_ID"
stream_send B "$DD" "$SB_LOG" "$SB_ID"
assert_no_panic "$SA_LOG" send-A; assert_no_panic "$SB_LOG" send-B
assert_d6_identity_busy_rejection
assert_messages_dir_clean
assert_peer_messages "$DD/chat/messages/$SA_ID.jsonl" A
assert_peer_messages "$DD/chat/messages/$SB_ID.jsonl" B
assert_history_view_matches_disk "$SA_ID" "$DD/chat/messages/$SA_ID.jsonl"
assert_history_view_matches_disk "$SB_ID" "$DD/chat/messages/$SB_ID.jsonl"

rm -f "$DD/.view.json" "$DD/.view2.json" "$DD/.hist.json"
echo "并发语义实测：好友簿 yrs CRDT 合并（无锁零静默丢失）；消息 JSONL 追加行级完整"
echo "N2-R2-OK"
