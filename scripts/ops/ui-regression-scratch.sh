#!/usr/bin/env bash
# U1 UI 回归批产：基于页面语义协议（GC3/GC4）对 8 路由逐页回归。
# 每页三步：1) gui navigate → gui page --json 断言 2) 动作级断言 3) gui screenshot PNG 证据。
# 动作断言模式：CONFIRM_NEG=危险动作缺 confirm 断言 ACTION_CONFIRM_REQUIRED 拒绝；
#   EXEC_STRUCT=只读动作真执行、以必拒绝参数取结构化 ACTION_FAILED（零网络零数据）；
#   REGISTRY_GAP=未注册页（当前注册表仅 chat/peers/settings）断言其结构化
#   PAGE_NOT_REGISTERED 拒绝——协议缺口按 R4 只报不改，负向断言计分。
# 用法：ui-regression.sh [--keep <dir>]。默认证据入临时目录退出全清（造数不过夜）；
#   --keep <dir> 保留全部截图与 report.txt。幂等：零数据写入（从不传 confirm=true）。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CTL="$ROOT/apps/cli/target/debug/p2pctl"
GUI_DIR="$ROOT/apps/gui"
GUI_BIN="$GUI_DIR/src-tauri/target/debug/p2p-console"
EP_FILE="$HOME/Library/Application Support/com.p2p.console/control/endpoint.json"
PNG_MAGIC="89504e470d0a1a0a"
PAGES_TOTAL=8
# 合成 PeerId（合法 base58）仅占参数位；本脚本不加好友、不复位身份、不写配置。
SYNTH_PEER="Cs8KY3PiWrCMAytMsBRQo8EdGbticVtdvufLnb2UhXh"

fail() { echo "UI-REG-ERROR {\"code\":\"$1\",\"message\":\"$2\"}" >&2; exit 2; }
fail_arg() { echo "UI-REG-ERROR {\"code\":\"ARG_INVALID\",\"message\":\"$1\"}" >&2; exit 2; }

KEEP_DIR=""
case "${1:-}" in
    "") ;;
    --keep)
        [ $# -ge 2 ] || fail_arg "--keep 需要目录参数"
        KEEP_DIR="$2" ;;
    *) fail_arg "未知参数 $1（支持 --keep <dir>）" ;;
esac

TMP="$(mktemp -d "/tmp/ui-regression.XXXXXX")"
GUI_LOG="$TMP/gui.log"
REPORT="$TMP/report.txt"
SHOT_DIR="$TMP"
if [ -n "$KEEP_DIR" ]; then
    # 证据直写指定目录：截图运行中即落盘、报告随 tee 直写；不靠退出时搬运
    # （搬运式 cp 一旦静默失败，--keep 就变成 0 图且无信号）。
    mkdir -p "$KEEP_DIR" || fail "KEEP_DIR_UNWRITABLE" "无法创建证据目录: $KEEP_DIR"
    SHOT_DIR="$KEEP_DIR"
    REPORT="$KEEP_DIR/report.txt"
fi
CHILD=""
EP_BACKUP=""
ASSERT_PASS=0
ASSERT_FAIL=0
PASSED_PAGES=0
FAILED_PAGES=0
FAILED_LIST=""
GAP_PAGES=""
PAGE_PASS=0
PAGE_FAIL=0
PAGE_REASON=""
PAGE_MODE=""
PAGE_TABLE=""

cleanup() {
    if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
        kill "$CHILD" 2>/dev/null || true
        for _ in $(seq 1 50); do
            kill -0 "$CHILD" 2>/dev/null || break
            sleep 0.2
        done
        kill -9 "$CHILD" 2>/dev/null || true
    fi
    # 端点文件只动本实例：有备份还原备份；否则 pid 匹配才删（不碰外部 GUI 的端点）。
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

a_pass() { ASSERT_PASS=$((ASSERT_PASS + 1)); PAGE_PASS=$((PAGE_PASS + 1)); }
a_fail() {
    ASSERT_FAIL=$((ASSERT_FAIL + 1)); PAGE_FAIL=$((PAGE_FAIL + 1))
    [ -n "$PAGE_REASON" ] || PAGE_REASON="$1"
}

# 必须成功的一步：失败记断言失败并回显输出（可观测），不中断整批（保逐页报告完整）。
must_ok() {
    local out rc
    set +e
    out="$("$@" 2>&1)"
    rc=$?
    set -e
    if [ "$rc" -ne 0 ]; then
        printf '%s\n' "$out" >&2
        a_fail "步骤失败（rc=$rc）: $*"
        return 0
    fi
    a_pass
    printf '%s' "$out"
}

# 期望拒绝的一步：断言非零退出且输出含指定结构化错误码（负向断言计分）。
expect_code() {
    local code="$1" out rc
    shift
    set +e
    out="$("$@" 2>&1)"
    rc=$?
    set -e
    if [ "$rc" -eq 0 ]; then
        a_fail "期望拒绝但成功: $*"
        return 0
    fi
    if printf '%s' "$out" | grep -q "$code"; then
        a_pass
    else
        a_fail "错误缺 $code（rc=$rc）: $(printf '%s' "$out" | tail -1)"
    fi
}

build_if_missing() {
    if [ ! -x "$CTL" ]; then
        echo "p2pctl 不存在，构建 apps/cli…" >&2
        (cd "$ROOT" && cargo build --manifest-path apps/cli/Cargo.toml) >&2
    fi
    if [ ! -f "$GUI_DIR/dist/index.html" ]; then
        [ -d "$GUI_DIR/node_modules" ] || (cd "$GUI_DIR" && pnpm install --frozen-lockfile) >&2
        echo "前端产物缺失，pnpm build…" >&2
        (cd "$GUI_DIR" && pnpm build) >&2
    fi
    if [ ! -x "$GUI_BIN" ]; then
        # tauri v2 plain cargo build 是 dev 态二进制（加载 devUrl），无 dev 服务器时
        # webview 空壳必 PAGE_TIMEOUT，必须 custom-protocol（cli-page-e2e.sh 实测）。
        echo "GUI 二进制缺失，cargo build src-tauri（custom-protocol）…" >&2
        (cd "$GUI_DIR/src-tauri" && cargo build --features tauri/custom-protocol) >&2
    fi
    [ -x "$CTL" ] || fail "BUILD_MISSING" "p2pctl 构建后仍不可执行: $CTL"
    [ -x "$GUI_BIN" ] || fail "BUILD_MISSING" "GUI 二进制构建后仍不可执行: $GUI_BIN"
}

start_gui() {
    # 健康外部实例直接复用：双实例并存会互写 endpoint（后初始化者胜），且外部
    # 进程不是本脚本的 cleanup 对象——杀外部 GUI 是事故。
    if [ -f "$EP_FILE" ]; then
        EPID="$(grep -Eo '"pid": [0-9]+' "$EP_FILE" 2>/dev/null | grep -Eo '[0-9]+')"
        if [ -n "$EPID" ] && ps -p "$EPID" >/dev/null 2>&1 \
            && "$CTL" gui status --json >/dev/null 2>&1; then
            echo "复用已运行 GUI 实例 pid=$EPID（外部进程不杀不复位）"
            return 0
        fi
    fi
    [ -f "$EP_FILE" ] && EP_BACKUP="$(cat "$EP_FILE")" || true
    "$GUI_BIN" >>"$GUI_LOG" 2>&1 &
    CHILD=$!
    local i
    for i in $(seq 1 240); do
        if [ -f "$EP_FILE" ] \
            && grep -Eq "\"pid\": $CHILD([^0-9]|$)" "$EP_FILE" \
            && "$CTL" gui status --json >/dev/null 2>&1; then
            echo "GUI 就绪 pid=$CHILD"
            return 0
        fi
        kill -0 "$CHILD" 2>/dev/null || break
        sleep 1
    done
    ps -p "$CHILD" -o pid,stat,etime,comm 2>/dev/null >&2 || true
    tail -20 "$GUI_LOG" >&2 || true
    fail "GUI_NOT_READY" "GUI 端点 240s 未就绪（pid=$CHILD），诊断见上方"
}

# descriptor 字段断言器：stdin=gui page --json 输出，逐字段独立计分。
check_field() {
    local route="$1" field="$2" json="$3"
    if node -e 'const[,r,f]=process.argv;let d;try{d=JSON.parse(require("fs").readFileSync(0,"utf8"))}catch{process.exit(1)}const c=d.descriptor||{};const ok={page:d.page===r,name:c.name===r,description:typeof c.description==="string"&&c.description.length>0,actions:Array.isArray(c.actions)&&c.actions.length>0,schema:d.schemaVersion===1}[f];process.exit(ok?0:1)' "$route" "$field" <<<"$json" >/dev/null 2>&1; then
        a_pass
    else
        a_fail "descriptor 断言失败: $field"
    fi
}

is_registered() { case "$1" in chat|peers|settings) return 0 ;; *) return 1 ;; esac; }

# navigate 后 route 归位轮询（≤5s）：控制通道 route 为异步传播，立即查询会读到
# 旧页（GUI 启动后首页必现竞态）；上界与 page 回执 5s 超时同一量级。
wait_route() {
    local route="$1" i
    for i in $(seq 1 20); do
        "$CTL" gui status --json 2>/dev/null | grep -q "\"route\": \"$route\"" && return 0
        sleep 0.25
    done
    return 1
}

assert_descriptor_full() {
    local route="$1" json f
    json="$(must_ok "$CTL" gui page --json)"
    for f in page name description actions schema; do
        check_field "$route" "$f" "$json"
    done
}

assert_action() {
    local route="$1"
    case "$route" in
        chat)
            expect_code ACTION_CONFIRM_REQUIRED \
                "$CTL" gui action chat removeFriend peer="$SYNTH_PEER" --navigate ;;
        peers)
            # peers 无 confirm 动作：只读 ping 以 timeoutMs=0 真执行到 store/IPC 校验层。
            PAGE_MODE="EXEC_STRUCT"
            expect_code ACTION_FAILED \
                "$CTL" gui action peers ping peerId="$SYNTH_PEER" timeoutMs=0 --navigate ;;
        settings)
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action settings resetIdentity --navigate ;;
        *)
            expect_code PAGE_NOT_REGISTERED "$CTL" gui action "$route" noop --navigate ;;
    esac
}

assert_screenshot() {
    local route="$1" shot="$SHOT_DIR/$route.png"
    must_ok "$CTL" gui screenshot -o "$shot" >/dev/null
    if [ -s "$shot" ]; then
        a_pass
    else
        a_fail "截图为空: $shot"
        return 0
    fi
    if [ "$(head -c 8 "$shot" | xxd -p)" = "$PNG_MAGIC" ]; then
        a_pass
    else
        a_fail "非 PNG magic: $shot"
    fi
}

run_page() {
    local route="$1" verdict row
    PAGE_PASS=0; PAGE_FAIL=0; PAGE_REASON=""; PAGE_MODE="CONFIRM_NEG"
    must_ok "$CTL" gui navigate "$route" >/dev/null
    wait_route "$route" || a_fail "route 5s 未归位: $route"
    if is_registered "$route"; then
        assert_descriptor_full "$route"
    else
        PAGE_MODE="REGISTRY_GAP"
        GAP_PAGES="$GAP_PAGES $route"
        # 协议缺口负向断言：未注册页必须结构化拒绝（而非崩溃/静默/超时）。
        expect_code PAGE_NOT_REGISTERED "$CTL" gui page --json
    fi
    assert_action "$route"
    assert_screenshot "$route"
    if [ "$PAGE_FAIL" -eq 0 ]; then
        verdict=PASS
        PASSED_PAGES=$((PASSED_PAGES + 1))
    else
        verdict=FAIL
        FAILED_PAGES=$((FAILED_PAGES + 1))
        FAILED_LIST="$FAILED_LIST $route"
    fi
    [ -n "$PAGE_REASON" ] || PAGE_REASON="-"
    row="$(printf '%-12s %-6s %2d/%-2d %-12s %s' \
        "$route" "$verdict" "$PAGE_PASS" "$((PAGE_PASS + PAGE_FAIL))" "$PAGE_MODE" "$PAGE_REASON")"
    PAGE_TABLE="$PAGE_TABLE$row
"
    echo "  [$route] $verdict 断言 $PAGE_PASS/$((PAGE_PASS + PAGE_FAIL))"
}

emit_report() {
    {
        echo "== U1 UI 回归报告（8 路由：navigate/page 断言 + 动作断言 + screenshot） =="
        echo "route        verdict pass/total mode         note"
        printf '%s' "$PAGE_TABLE"
        echo "-- 模式: CONFIRM_NEG=危险动作缺confirm被拒 EXEC_STRUCT=只读动作真执行取结构化拒绝 REGISTRY_GAP=未注册页结构化拒绝 --"
        echo "-- 协议缺口（R4 只报不改，待协调者另立卡）: 注册表仅登记 chat/peers/settings --"
        echo "GAP_PAGES:$GAP_PAGES"
        echo "SUMMARY: pages=$PAGES_TOTAL passed=$PASSED_PAGES failed=$FAILED_PAGES assertions=$ASSERT_PASS/$((ASSERT_PASS + ASSERT_FAIL))"
        echo "EVIDENCE: keep=$KEEP_DIR dir=$SHOT_DIR pngs=$(ls "$SHOT_DIR" | grep -c '\.png$' || true)"
        if [ "$FAILED_PAGES" -eq 0 ]; then
            echo "UI-REG-OK"
        else
            echo "UI-REG-FAIL 失败页:$FAILED_LIST（原因见上表 note 列）"
        fi
    } | tee "$REPORT"
}

main() {
    build_if_missing
    start_gui
    echo "== U1 UI 回归开始（证据目录 $SHOT_DIR，keep=$KEEP_DIR） =="
    local r
    for r in chat dashboard diagnostics discovery events peers relay settings; do
        run_page "$r"
    done
    emit_report
    [ "$FAILED_PAGES" -eq 0 ] || exit 1
}

main "$@"
