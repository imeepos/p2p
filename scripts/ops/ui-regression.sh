#!/usr/bin/env bash
# U1 UI 回归批产：新外壳路由集逐页回归（navigate+descriptor+动作断言+截图证据）：
# /chat、/settings、/network 六 tab，含 /、/group、/acp 三条重定向落点断言与 /contacts 缺口负向断言。
# 控制通道白名单（src-tauri control/mod.rs ROUTES）与 PAGE_REGISTRY 键仍为旧注册名
# （dashboard=overview，见 control-bridge ROUTE_ALIASES）：六 tab 经旧路由重定向进入，
# 落点以 health.route 归一化、descriptor、截图三重证明；"/" 落点即 dashboard 行（index 重定向）。
# CONFIRM_NEG=危险动作缺 confirm 被拒；EXEC_STRUCT=非法参数真执行取结构化拒绝（零写入）。用法：[--keep <dir>]；幂等零持久写入，confirm=true 唯一豁免 events.clear（易失缓冲，幂等）。
set -eu -o pipefail
export PATH="$HOME/.cargo/bin:$PATH"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; CTL="$ROOT/apps/cli/target/debug/p2pctl"
GUI_DIR="$ROOT/apps/gui"; GUI_BIN="$GUI_DIR/src-tauri/target/debug/p2p-console"
EP_FILE="$HOME/Library/Application Support/com.p2p.console/control/endpoint.json"; PNG_MAGIC="89504e470d0a1a0a"
PAGES_TOTAL=11
SYNTH_PEER="Cs8KY3PiWrCMAytMsBRQo8EdGbticVtdvufLnb2UhXh" # 合成 PeerId 仅占参数位；不加好友不复位身份不写配置

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

TMP="$(mktemp -d "/tmp/ui-regression.XXXXXX")"; GUI_LOG="$TMP/gui.log"
REPORT="$TMP/report.txt"; SHOT_DIR="$TMP"
if [ -n "$KEEP_DIR" ]; then
    # 证据直写指定目录：截图运行中即落盘、报告随 tee 直写，不靠退出时搬运（cp 静默失败会变 0 图）。
    mkdir -p "$KEEP_DIR" || fail "KEEP_DIR_UNWRITABLE" "无法创建证据目录: $KEEP_DIR"
    SHOT_DIR="$(cd "$KEEP_DIR" && pwd)"; REPORT="$SHOT_DIR/report.txt"  # gui screenshot 仅收绝对路径
fi
CHILD=""; EP_BACKUP=""
ASSERT_PASS=0; ASSERT_FAIL=0; PASSED_PAGES=0; FAILED_PAGES=0; FAILED_LIST=""
PAGE_PASS=0; PAGE_FAIL=0; PAGE_REASON=""; PAGE_MODE=""; PAGE_TABLE=""

cleanup() {
    if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
        kill "$CHILD" 2>/dev/null || true
        for _ in $(seq 1 50); do
            kill -0 "$CHILD" 2>/dev/null || break
            sleep 0.2
        done
        kill -9 "$CHILD" 2>/dev/null || true
    fi
    wait "$CHILD" 2>/dev/null || true  # 显式回收丢弃终止状态，防信号泄进退出码
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
a_fail() { ASSERT_FAIL=$((ASSERT_FAIL + 1)); PAGE_FAIL=$((PAGE_FAIL + 1)); [ -n "$PAGE_REASON" ] || PAGE_REASON="$1"; }

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
    # dist 缺失或 src/配置比产物新即重建（mtime 一线检测）：回归测旧前端整场假红/假绿
    if [ ! -f "$GUI_DIR/dist/index.html" ] \
        || [ -n "$(find "$GUI_DIR/src" "$GUI_DIR/package.json" -newer "$GUI_DIR/dist/index.html" -print -quit 2>/dev/null)" ]; then
        [ -d "$GUI_DIR/node_modules" ] || (cd "$GUI_DIR" && pnpm install --frozen-lockfile) >&2
        echo "前端产物缺失或过期，pnpm build…" >&2
        (cd "$GUI_DIR" && pnpm build) >&2
    fi
    # bin 必须是 custom-protocol 态（dev 态加载 devUrl 即空壳）；无条件按该形态构建，裁决交 cargo fingerprint。
    (cd "$GUI_DIR/src-tauri" && cargo build --features tauri/custom-protocol) >&2
    [ -x "$CTL" ] || fail "BUILD_MISSING" "p2pctl 构建后仍不可执行: $CTL"
    [ -x "$GUI_BIN" ] || fail "BUILD_MISSING" "GUI 二进制构建后仍不可执行: $GUI_BIN"
}

start_gui() {
    # 健康外部实例直接复用（双实例会互写 endpoint；外部进程不杀——杀外部 GUI 是事故）。
    if [ -f "$EP_FILE" ]; then
        EPID="$(grep -Eo '"pid": [0-9]+' "$EP_FILE" 2>/dev/null | grep -Eo '[0-9]+')"
        if [ -n "$EPID" ] && ps -p "$EPID" >/dev/null 2>&1 \
            && "$CTL" gui status --json >/dev/null 2>&1 \
            && "$CTL" gui page --json >/dev/null 2>&1; then
            echo "复用已运行 GUI 实例 pid=$EPID（外部进程不杀不复位）"
            return 0
        fi
    fi
    [ -f "$EP_FILE" ] && EP_BACKUP="$(cat "$EP_FILE")" || true
    "$GUI_BIN" >>"$GUI_LOG" 2>&1 &
    CHILD=$!; disown "$CHILD" 2>/dev/null || true
    # 就绪门探针=真实 round-trip 成功（gui page --json 退出码 0）：端点就绪 ≠ 桥就绪，
    # 仅排除 PAGE_TIMEOUT 字样会被其他错误形态提前放行（GC3c 实测）。单次 ≤5s。
    for i in $(seq 1 60); do
        if [ -f "$EP_FILE" ] \
            && grep -Eq "\"pid\": $CHILD([^0-9]|$)" "$EP_FILE" \
            && "$CTL" gui status --json >/dev/null 2>&1 \
            && "$CTL" gui page --json >/dev/null 2>&1; then
            echo "GUI 就绪 pid=$CHILD"
            return 0
        fi
        kill -0 "$CHILD" 2>/dev/null || break; sleep 1
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

# navigate 后 route 异步传播，轮询 ≤5s 归位（立即查询会读到旧页，上界与 page 回执超时同量级）。
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
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action chat removeFriend peer="$SYNTH_PEER" --navigate ;;
        peers) # 无 confirm 动作：只读 ping 以 timeoutMs=0 真执行到 store/IPC 校验层
            PAGE_MODE="EXEC_STRUCT"
            expect_code ACTION_FAILED "$CTL" gui action peers ping peerId="$SYNTH_PEER" timeoutMs=0 --navigate ;;
        settings)
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action settings resetIdentity --navigate ;;
        dashboard)
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action dashboard stop --navigate ;;
        diagnostics) # HAPPY_PATH：refresh 只读 IPC 真执行取结构化回包
            PAGE_MODE="HAPPY_PATH"
            must_ok "$CTL" gui action diagnostics refresh tailLines=5 --navigate >/dev/null
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action diagnostics clearAll --navigate ;;
        discovery)
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action discovery removeBootstrap --navigate ;;
        events) # HAPPY_PATH：clear 带 confirm 真执行（易失缓冲，清后幂等）
            PAGE_MODE="HAPPY_PATH"
            must_ok "$CTL" gui action events clear confirm=true --navigate >/dev/null
            expect_code ACTION_CONFIRM_REQUIRED "$CTL" gui action events clear --navigate ;;
        relay) # EXEC_STRUCT：非法地址在校验层结构化拒绝，零写入真执行
            PAGE_MODE="EXEC_STRUCT"
            expect_code ACTION_FAILED "$CTL" gui action relay saveRelayAddrs 'relayAddrs=["invalid-addr"]' --navigate ;;
        *) a_fail "route 缺动作断言: $route" ;;
    esac
}

assert_screenshot() {
    # bash 3.2(set -u)：local 同语句依赖前置赋值会读到调用方作用域，须分号隔成两步。
    local route="$1"; local shot="$SHOT_DIR/$route.png"
    must_ok "$CTL" gui screenshot -o "$shot" >/dev/null
    if [ -s "$shot" ] && [ "$(head -c 8 "$shot" | xxd -p)" = "$PNG_MAGIC" ]; then
        a_pass
    else
        a_fail "截图缺失或非 PNG: $shot"
    fi
}

# 行计数汇总：verdict/断言比分/表格行，run_page/run_redirect/run_gap 共用。
finalize_page() {
    local label="$1" verdict
    if [ "$PAGE_FAIL" -eq 0 ]; then
        verdict=PASS
        PASSED_PAGES=$((PASSED_PAGES + 1))
    else
        verdict=FAIL
        FAILED_PAGES=$((FAILED_PAGES + 1))
        FAILED_LIST="$FAILED_LIST $label"
    fi
    [ -n "$PAGE_REASON" ] || PAGE_REASON="-"
    PAGE_TABLE="$PAGE_TABLE$(printf '%-16s %-6s %2d/%-2d %-12s %s' \
        "$label" "$verdict" "$PAGE_PASS" "$((PAGE_PASS + PAGE_FAIL))" "$PAGE_MODE" "$PAGE_REASON")
"
    echo "  [$label] $verdict 断言 $PAGE_PASS/$((PAGE_PASS + PAGE_FAIL))"
}

run_page() {
    local route="$1"
    PAGE_PASS=0; PAGE_FAIL=0; PAGE_REASON=""; PAGE_MODE="CONFIRM_NEG"
    must_ok "$CTL" gui navigate "$route" >/dev/null
    wait_route "$route" || a_fail "route 5s 未归位: $route"
    assert_descriptor_full "$route"
    assert_action "$route"
    assert_screenshot "$route"
    finalize_page "$route"
}

# 重定向落点断言：navigate 旧路由源，断言落点归一化路由、descriptor 页名与截图。
run_redirect() {
    local source="$1" target="$2"
    PAGE_PASS=0; PAGE_FAIL=0; PAGE_REASON=""; PAGE_MODE="REDIRECT"
    must_ok "$CTL" gui navigate "$source" >/dev/null
    wait_route "$target" || a_fail "重定向落点 5s 未归位: $source -> $target"
    assert_descriptor_full "$target"
    assert_screenshot "redirect-$source"
    finalize_page "$source->$target"
}

# /contacts 缺口负向断言（不静默跳过）：白名单与注册表均未登记，navigate 必 INVALID_ROUTE；
# 日后补登记会让本行转红，倒逼纳入正向回归。
run_gap_contacts() {
    PAGE_PASS=0; PAGE_FAIL=0; PAGE_REASON=""; PAGE_MODE="GAP-PINNED"
    expect_code INVALID_ROUTE "$CTL" gui navigate contacts
    finalize_page "contacts(gap)"
}

emit_report() {
    {
        echo "== U1 UI 回归报告（新外壳 11 行：8 descriptor 页 + 2 重定向落点 + 1 缺口断言） =="
        echo "route            verdict pass/total mode         note"
        printf '%s' "$PAGE_TABLE"
        echo "SUMMARY: pages=$PAGES_TOTAL passed=$PASSED_PAGES failed=$FAILED_PAGES assertions=$ASSERT_PASS/$((ASSERT_PASS + ASSERT_FAIL))"
        echo "EVIDENCE: keep=$KEEP_DIR dir=$SHOT_DIR pngs=$(ls "$SHOT_DIR" | grep -c '\.png$' || true)"
        echo "NOTES: /group /acp 落点 /chat?kind=*；contacts 为登记缺口（GAP-PINNED 钉住，不静默跳过）。"
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
    for r in dashboard peers discovery relay events diagnostics chat settings; do
        run_page "$r"
    done
    run_redirect group chat
    run_redirect acp chat
    run_gap_contacts
    emit_report
    if [ "$FAILED_PAGES" -eq 0 ]; then
        exit 0
    fi
    exit 1
}

main "$@"
