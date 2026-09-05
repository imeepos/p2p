#!/usr/bin/env bash
# GC4 E2E：p2pctl gui page/action × 真实 GUI 页面语义协议（后续 UI 测试的模板）。
# 流程：构建 CLI 与 GUI → 后台启动真实 GUI → 轮询端点就绪 → screenshot BEFORE →
#   navigate chat → gui page 断言非空 actions（文本+JSON 双形态）→
#   chat.addFriend 两连（幂等回包）→ removeFriend confirm=true 断言删除 →
#   非当前页结构化报错（含 gui navigate 指引）→ --navigate 切页后缺 confirm 的
#   危险动作透传 ACTION_CONFIRM_REQUIRED（不真执行）→ screenshot AFTER（前后各
#   一张非空 PNG，UI 证据）→ 清理造数与进程（造数不过夜）→ 末行 GC4-E2E-OK。
# 幂等：可重复执行；已有 GUI 实例运行时备份 endpoint.json、以 pid 匹配本实例、
#   退出后还原；合成好友（全零 PeerId）收尾必删，异常路径 best-effort 补删。
set -eu
set -o pipefail
# cargo 不在默认 PATH（acceptance 链会导出；脚本自包含双保险）
export PATH="$HOME/.cargo/bin:$PATH"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="$ROOT/apps/cli/target/debug/p2pctl"
GUI_DIR="$ROOT/apps/gui"
GUI_BIN="$GUI_DIR/src-tauri/target/debug/p2p-console"
DATA_DIR="${HOME}/Library/Application Support/com.p2p.console"
EP_FILE="$DATA_DIR/control/endpoint.json"
# 合成 PeerId：合法 32 字节 base58（含字母——纯数字串会被 k=v 基础类型解析成 number），
# 仅占地址簿，无网络语义。Cs8KY3...由 bytes((i*7+3)%256 for i in range(32)) 经 base58 编码而来。
PEER_ID="Cs8KY3PiWrCMAytMsBRQo8EdGbticVtdvufLnb2UhXh"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/gc4-page-e2e.XXXXXX")"
GUI_LOG="$TMP/gui.log"
CHILD=""
EP_BACKUP=""

fail() { echo "GC4-E2E FAIL: $1" >&2; exit 1; }

cleanup() {
    # best-effort 造数回收：GUI 仍存活时删合成好友（幂等，失败留告警不掩盖主流程）
    if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null \
        && [ -x "$CTL" ] && "$CTL" gui status --json >/dev/null 2>&1; then
        "$CTL" gui action chat removeFriend peer="$PEER_ID" confirm=true --navigate >/dev/null 2>&1 || \
            echo "GC4-E2E WARN: 合成好友收尾删除失败（peer=$PEER_ID）" >&2
    fi
    if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
        kill "$CHILD" 2>/dev/null || true
        for _ in $(seq 1 50); do
            kill -0 "$CHILD" 2>/dev/null || break
            sleep 0.2
        done
        kill -9 "$CHILD" 2>/dev/null || true
    fi
    if [ -n "$EP_BACKUP" ]; then
        mkdir -p "$(dirname "$EP_FILE")"
        printf '%s' "$EP_BACKUP" > "$EP_FILE"
    elif [ -n "$CHILD" ] && [ -f "$EP_FILE" ] \
        && grep -Eq "\"pid\": $CHILD([^0-9]|$)" "$EP_FILE" 2>/dev/null; then
        rm -f "$EP_FILE"
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT

# 跑一步原语；非零退出时先原样回显错误（可见），含权限语义再输出提示，然后失败。
run_guarded() {
    local out rc
    set +e
    out="$("$@" 2>&1)"
    rc=$?
    set -e
    if [ "$rc" -ne 0 ]; then
        printf '%s\n' "$out" >&2
        if printf '%s' "$out" | grep -qiE 'PERMISSION|权限'; then
            echo "GC4-E2E: 需要 macOS 屏幕录制权限（系统设置→隐私与安全性）" >&2
        fi
        fail "步骤失败（rc=$rc）: $*"
    fi
    printf '%s' "$out"
}

# 期望失败的一步：断言非零退出且错误含指定语义串（结构化拒绝可观测）。
expect_fail() {
    local pat="$1"; shift
    local out rc
    set +e
    out="$("$@" 2>&1)"
    rc=$?
    set -e
    if [ "$rc" -eq 0 ]; then fail "期望失败但成功: $*"; fi
    printf '%s\n' "$out" >&2
    printf '%s\n' "$out" | grep -q "$pat" || fail "错误缺「$pat」（rc=$rc）: $out"
}

echo "== 1. 构建 CLI 与 GUI（缺失才全量构建，增量复用） =="
if [ ! -x "$CTL" ]; then
    echo "p2pctl 不存在，先构建 apps/cli…" >&2
    (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2
fi
if [ ! -f "$GUI_DIR/dist/index.html" ]; then
    [ -d "$GUI_DIR/node_modules" ] || (cd "$GUI_DIR" && pnpm install --frozen-lockfile) >&2
    echo "前端产物缺失，pnpm build…" >&2
    (cd "$GUI_DIR" && pnpm build) >&2
fi
if [ ! -x "$GUI_BIN" ]; then
    # custom-protocol 必开：tauri v2 下 plain cargo build 是 dev 态二进制（加载 devUrl），
    # 无 dev 服务器时 webview 空壳，页面桥不存在必 PAGE_TIMEOUT（预验证实测）。
    echo "GUI 二进制缺失，cargo build src-tauri（custom-protocol）…" >&2
    (cd "$GUI_DIR/src-tauri" && cargo build --features tauri/custom-protocol) >&2
fi
[ -x "$GUI_BIN" ] || fail "GUI 二进制未就绪: $GUI_BIN"

echo "== 2. 启动真实 GUI（后台）并轮询端点就绪 =="
[ -f "$EP_FILE" ] && EP_BACKUP="$(cat "$EP_FILE")"
"$GUI_BIN" >>"$GUI_LOG" 2>&1 &
CHILD=$!
READY=0
for _ in $(seq 1 90); do
    if [ -f "$EP_FILE" ] \
        && grep -Eq "\"pid\": $CHILD([^0-9]|$)" "$EP_FILE" \
        && "$CTL" gui status --json >/dev/null 2>&1; then
        READY=1
        break
    fi
    kill -0 "$CHILD" 2>/dev/null || break
    sleep 1
done
[ "$READY" -eq 1 ] || { tail -20 "$GUI_LOG" >&2; fail "GUI 端点 90s 未就绪（pid=$CHILD）"; }
echo "GUI 就绪 pid=$CHILD endpoint=$EP_FILE"

echo "== 3. screenshot BEFORE（前证据，非空 PNG） =="
SHOT_BEFORE="$TMP/shot-before.png"
run_guarded "$CTL" gui screenshot -o "$SHOT_BEFORE" >/dev/null
[ -s "$SHOT_BEFORE" ] || fail "before 截图为空: $SHOT_BEFORE"
head -c 8 "$SHOT_BEFORE" | xxd -p | grep -qi "^89504e47" || fail "before 非 PNG magic: $SHOT_BEFORE"
echo "before=$(wc -c < "$SHOT_BEFORE" | tr -d ' ') 字节 PNG"

echo "== 4. navigate chat + gui page 断言非空 actions =="
run_guarded "$CTL" gui navigate chat >/dev/null
PAGE_TEXT="$(run_guarded "$CTL" gui page)"
printf '%s\n' "$PAGE_TEXT" | grep -q '^name=chat' || fail "page 文本缺 name=chat: $PAGE_TEXT"
printf '%s\n' "$PAGE_TEXT" | grep -q '^description=.' || fail "page 文本缺 description: $PAGE_TEXT"
printf '%s\n' "$PAGE_TEXT" | grep -Eq '^actions=[1-9]' || fail "page 文本 actions 为空: $PAGE_TEXT"
printf '%s\n' "$PAGE_TEXT" | grep -q '^- sendText:' || fail "page 文本缺动作行: $PAGE_TEXT"
echo "$PAGE_TEXT"
PAGE_JSON="$(run_guarded "$CTL" gui page --json)"
printf '%s\n' "$PAGE_JSON" | grep -A1 '"actions": \[' | grep -q '{' || fail "page JSON actions 非空断言失败: $PAGE_JSON"
printf '%s\n' "$PAGE_JSON" | grep -q '"schemaVersion"' || fail "page JSON 缺 schemaVersion: $PAGE_JSON"

echo "== 5. saveAndRestart 启动节点（confirm=true 正面用例）→ 好友增删 =="
# chat 域动作要求 GUI 内节点运行；注册表无裸 node start，真实用户流即 saveAndRestart。
# config 先经 invoke config_get 读真实值再原样写回：等值写零数据变更（造数安全）。
CFG=$($CTL gui invoke config_get --json | node -e "let d='';process.stdin.on('data',c=>d+=c).on('end',()=>process.stdout.write(JSON.stringify(JSON.parse(d).result)))")
RESTART="$(run_guarded "$CTL" gui action settings saveAndRestart config="$CFG" confirm=true --navigate --json)"
printf '%s\n' "$RESTART" | grep -q '"running": true' || fail "saveAndRestart 回包异常: $RESTART"
echo "saveAndRestart（confirm=true）节点已启动"
run_guarded "$CTL" gui navigate chat >/dev/null
ADD1="$(run_guarded "$CTL" gui action chat addFriend peerId="$PEER_ID" nickname=gc4-e2e --json)"
printf '%s\n' "$ADD1" | grep -q '"requestId"' || fail "addFriend 回包缺 requestId 信封: $ADD1"
printf '%s\n' "$ADD1" | grep -q '"peerId"' || fail "addFriend 回包缺 peerId: $ADD1"
ADD2="$(run_guarded "$CTL" gui action chat addFriend peerId="$PEER_ID" nickname=gc4-e2e --json)"
printf '%s\n' "$ADD2" | grep -q '"peerId"' || fail "addFriend 幂等二连失败: $ADD2"
echo "addFriend 两连幂等回包 OK"
RM="$(run_guarded "$CTL" gui action chat removeFriend peer="$PEER_ID" confirm=true --json)"
printf '%s\n' "$RM" | grep -q '"removed"' || fail "removeFriend 回包异常: $RM"

echo "== 6. 非当前页结构化报错（含 gui navigate 指引） =="
expect_fail "gui navigate settings" "$CTL" gui action settings resetIdentity

echo "== 7. --navigate 切页后危险动作缺 confirm 透传拒绝（不真执行） =="
expect_fail "ACTION_CONFIRM_REQUIRED" "$CTL" gui action settings resetIdentity --navigate
CUR="$(run_guarded "$CTL" gui status --json)"
printf '%s\n' "$CUR" | grep -q '"route": "settings"' || fail "--navigate 未切页: $CUR"

echo "== 8. screenshot AFTER（后证据，非空 PNG） =="
SHOT_AFTER="$TMP/shot-after.png"
run_guarded "$CTL" gui screenshot -o "$SHOT_AFTER" >/dev/null
[ -s "$SHOT_AFTER" ] || fail "after 截图为空: $SHOT_AFTER"
head -c 8 "$SHOT_AFTER" | xxd -p | grep -qi "^89504e47" || fail "after 非 PNG magic: $SHOT_AFTER"
echo "after=$(wc -c < "$SHOT_AFTER" | tr -d ' ') 字节 PNG"

echo "GC4-E2E-OK"
