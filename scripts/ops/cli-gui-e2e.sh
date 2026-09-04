#!/usr/bin/env bash
# GC2 E2E：p2pctl gui 域 × 真实 GUI 控制通道全链路（真机联动）。
# 流程：构建 CLI 与 GUI → 后台启动真实 GUI → 轮询端点状态文件就绪 →
#   status 断言 → navigate chat 断言路由 → screenshot 非空 PNG →
#   record start/stop 产物 GIF → invoke 白名单回包 → 未运行结构化错误 →
#   清理 GUI 进程与全部临时文件（造数不过夜）→ 末行 GC2-E2E-OK。
# 幂等：可重复执行；已有 GUI 实例运行时先备份 endpoint.json、以 pid 匹配
#   本实例端点、退出后还原备份（另一实例的可发现性不受损）。
# 权限提示（R4）：screenshot/record 失败且错误含权限语义时，输出可读提示
#   后再失败——失败可见，禁止跳过。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CTL="$ROOT/apps/cli/target/debug/p2pctl"
GUI_DIR="$ROOT/apps/gui"
GUI_BIN="$GUI_DIR/src-tauri/target/debug/p2p-console"
DATA_DIR="${HOME}/Library/Application Support/com.p2p.console"
EP_FILE="$DATA_DIR/control/endpoint.json"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/gc2-gui-e2e.XXXXXX")"
GUI_LOG="$TMP/gui.log"
CHILD=""
EP_BACKUP=""

fail() { echo "GC2-E2E FAIL: $1" >&2; exit 1; }

cleanup() {
    if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
        kill "$CHILD" 2>/dev/null || true
        for _ in $(seq 1 50); do
            kill -0 "$CHILD" 2>/dev/null || break
            sleep 0.2
        done
        kill -9 "$CHILD" 2>/dev/null || true
    fi
    # 我们的实例退出后：还原启动前备份（另一实例仍可被发现）；
    # 无备份则摘除指向本实例 pid 的残留端点文件（SIGKILL 不走 RunEvent::Exit）。
    if [ -n "$EP_BACKUP" ]; then
        mkdir -p "$(dirname "$EP_FILE")"
        printf '%s' "$EP_BACKUP" > "$EP_FILE"
    elif [ -n "$CHILD" ] && [ -f "$EP_FILE" ] && grep -Eq "\"pid\": $CHILD([^0-9]|$)" "$EP_FILE" 2>/dev/null; then
        rm -f "$EP_FILE"
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT

# 跑一步原语；非零退出时先原样回显错误（可见），含权限语义再输出 R4 提示，然后失败。
run_guarded() {
    local out rc
    set +e
    out="$("$@" 2>&1)"
    rc=$?
    set -e
    if [ "$rc" -ne 0 ]; then
        printf '%s\n' "$out" >&2
        if printf '%s' "$out" | grep -qiE 'PERMISSION|权限'; then
            echo "GC2-E2E: 需要 macOS 屏幕录制权限（系统设置→隐私与安全性）" >&2
        fi
        fail "步骤失败（rc=$rc）: $*"
    fi
    printf '%s' "$out"
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
    echo "GUI 二进制缺失，cargo build src-tauri…" >&2
    (cd "$GUI_DIR/src-tauri" && cargo build) >&2
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

echo "== 3. gui status 断言（版本/窗口/路由） =="
STATUS_JSON="$(run_guarded "$CTL" gui status --json)"
printf "%s\n" "$STATUS_JSON" | grep -q '"version"' || fail "status JSON 缺 version: $STATUS_JSON"
printf "%s\n" "$STATUS_JSON" | grep -q '"title"' || fail "status JSON 缺 title: $STATUS_JSON"
printf "%s\n" "$STATUS_JSON" | grep -q "\"pid\": $CHILD" || fail "status pid 与本实例不符: $STATUS_JSON"
STATUS_TEXT="$(run_guarded "$CTL" gui status)"
printf "%s\n" "$STATUS_TEXT" | grep -q "^version=" || fail "status 文本缺 version="
printf "%s\n" "$STATUS_TEXT" | grep -q "^window=" || fail "status 文本缺 window="
printf "%s\n" "$STATUS_TEXT" | grep -q "^route=" || fail "status 文本缺 route="
echo "$STATUS_TEXT"

echo "== 4. gui navigate 切路由（peers → chat）并以 status 断言 =="
run_guarded "$CTL" gui navigate peers >/dev/null
ROUTE_PEERS="$(run_guarded "$CTL" gui status --json)"
printf "%s\n" "$ROUTE_PEERS" | grep -q '"route": "peers"' || fail "navigate peers 未生效: $ROUTE_PEERS"
NAV_OUT="$(run_guarded "$CTL" gui navigate chat)"
printf "%s\n" "$NAV_OUT" | grep -q "^route=chat" || fail "navigate chat 输出异常: $NAV_OUT"
ROUTE_CHAT="$(run_guarded "$CTL" gui status --json)"
printf "%s\n" "$ROUTE_CHAT" | grep -q '"route": "chat"' || fail "navigate chat 后路由非 chat: $ROUTE_CHAT"
echo "路由切换 peers→chat 断言通过"

echo "== 5. gui screenshot 非空 PNG =="
SHOT="$TMP/shot.png"
run_guarded "$CTL" gui screenshot -o "$SHOT" >/dev/null
[ -s "$SHOT" ] || fail "screenshot 产物为空或不存在: $SHOT"
head -c 8 "$SHOT" | xxd -p | grep -qi "^89504e47" || fail "产物非 PNG magic: $SHOT"
echo "screenshot 产物 $(wc -c < "$SHOT" | tr -d " ") 字节 PNG"

echo "== 6. gui record start→stop 产物 GIF =="
REC="$TMP/rec.gif"
run_guarded "$CTL" gui record start -o "$REC" >/dev/null
sleep 2
REC_OUT="$(run_guarded "$CTL" gui record stop)"
printf "%s\n" "$REC_OUT" | grep -q "^path=" || fail "record stop 输出缺 path=: $REC_OUT"
[ -s "$REC" ] || fail "record 产物为空或不存在: $REC"
head -c 6 "$REC" | xxd -p | grep -qi "^1f4946" || fail "产物非 GIF magic: $REC"
echo "record 产物 $(wc -c < "$REC" | tr -d " ") 字节 GIF"

echo "== 7. gui invoke 白名单回包 + 越权拒绝 =="
INVOKE_OUT="$(run_guarded "$CTL" gui invoke node_status --json)"
printf "%s\n" "$INVOKE_OUT" | grep -q '"result"' || fail "invoke 缺 result 回包: $INVOKE_OUT"
set +e
INVOKE_BAD="$("$CTL" gui invoke config_save 2>&1)"
BAD_RC=$?
set -e
[ "$BAD_RC" -ne 0 ] || fail "非白名单命令 config_save 竟然成功: $INVOKE_BAD"
printf "%s\n" "$INVOKE_BAD" | grep -q "INVOKE_FORBIDDEN" || fail "越权错误缺结构化码: $INVOKE_BAD"
echo "invoke 白名单转发与 403 拒绝均符合预期"

echo "== 8. GUI 未运行时结构化错误（隔离数据目录） =="
set +e
DIS_ERR="$("$CTL" gui status --gui-data-dir "$TMP/empty-data" 2>&1)"
DIS_RC=$?
set -e
[ "$DIS_RC" -eq 1 ] || fail "GUI 未运行时应退出码 1，实际 $DIS_RC: $DIS_ERR"
printf "%s\n" "$DIS_ERR" | grep -q "请先启动 GUI" || fail "结构化错误缺启动指引: $DIS_ERR"
echo "$DIS_ERR"

echo "GC2-E2E-OK"
