#!/usr/bin/env bash
# updater 签名预检：发布前机械验证本地签名环境（2026-09-05 client-v0.1.4/0.1.5 四平台
# 同挂「Tauri 打包」步的教训——环境不齐时打包步必挂，见 docs/ops/updater-release.md 对照表）。
# 用法：
#   bash scripts/ops/updater-signing-preflight.sh               # 检查当前 shell 环境
#   bash scripts/ops/updater-signing-preflight.sh --self-test   # 红绿双向自检，防假绿
# 退出码：0=全绿；1=存在红项（每个红项输出一行 FAIL + 对应真实报错）。
set -u
set -o pipefail

FAILS=0
PASS() { echo "PASS $1"; }
FAIL() { echo "FAIL $1 :: $2"; FAILS=$((FAILS+1)); }

# 检查输入统一走变量：主流程从真实环境取值，self-test 注入 fixture。
KEY_PATH_ENV=""
KEY_ENV=""
PWD_EXPORTED=0
PWD_VALUE=""

gather_from_env() {
  KEY_PATH_ENV="${TAURI_SIGNING_PRIVATE_KEY_PATH:-}"
  KEY_ENV="${TAURI_SIGNING_PRIVATE_KEY:-}"
  if [ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" ]; then
    PWD_EXPORTED=1
    PWD_VALUE="$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
  fi
}

# $1=密钥文件路径；回显解码后的内容头，不可读/非 base64 时回显空串并置 reason。
decode_key_header() {
  python3 - "$1" <<'PYEOF'
import sys
try:
    raw = open(sys.argv[1], 'rb').read()
except OSError:
    print(''); raise SystemExit
alpha = set(b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=')
if not raw or any(b not in alpha for b in raw):
    print(''); raise SystemExit
import base64
try:
    sys.stdout.write(base64.b64decode(raw, validate=True).decode('utf-8', 'replace')[:64])
except Exception:
    print('')
PYEOF
}

run_checks() {
  # 1 密钥路径变量
  if [ -n "$KEY_PATH_ENV" ]; then PASS "TAURI_SIGNING_PRIVATE_KEY_PATH 已设置"; else
    FAIL "key-path" "TAURI_SIGNING_PRIVATE_KEY_PATH 未设置（.env 已登记，source .env 后重试）"; fi
  # 2 密钥文件可读
  if [ -n "$KEY_PATH_ENV" ] && [ -r "$KEY_PATH_ENV" ] && [ -s "$KEY_PATH_ENV" ]; then
    PASS "密钥文件可读: $KEY_PATH_ENV"
  else
    FAIL "key-file" "密钥文件不可读或为空: $KEY_PATH_ENV"; fi
  # 3+4 base64 语义与密钥头
  local header=""
  if [ -n "$KEY_PATH_ENV" ] && [ -r "$KEY_PATH_ENV" ]; then
    header="$(decode_key_header "$KEY_PATH_ENV")"
    if [ -n "$header" ]; then PASS "密钥内容为严格 base64 且可解码"; else
      FAIL "key-base64" "密钥文件不是合法 base64（含空白/换行/杂字符）——CI 实证报错: Invalid symbol 37, offset 348"; fi
    case "$header" in
      "untrusted comment: rsign encrypted secret key"*)
        PASS "密钥头语义正确（rsign encrypted secret key）" ;;
      *) FAIL "key-header" "密钥头不是 rsign encrypted secret key，实际: ${header:0:48}" ;;
    esac
  else
    FAIL "key-base64" "密钥文件不可读，跳过 base64 检查"
    FAIL "key-header" "密钥文件不可读，跳过密钥头检查"
  fi
  # 5 KEY 导出且与文件逐字节一致
  if [ -n "$KEY_ENV" ]; then PASS "TAURI_SIGNING_PRIVATE_KEY 已导出"; else
    FAIL "key-env" "TAURI_SIGNING_PRIVATE_KEY 未导出（export TAURI_SIGNING_PRIVATE_KEY=\"\$(cat \"\$TAURI_SIGNING_PRIVATE_KEY_PATH\")\"）"; fi
  if [ -n "$KEY_ENV" ] && [ -n "$KEY_PATH_ENV" ] && [ -r "$KEY_PATH_ENV" ]; then
    if [ "$KEY_ENV" = "$(cat "$KEY_PATH_ENV")" ]; then PASS "KEY 环境变量与密钥文件逐字节一致"
    else FAIL "key-env-mismatch" "KEY 环境变量与密钥文件内容不一致（粘贴/截断）"; fi
  fi
  # 6 PASSWORD 必须显式导出（哪怕空串）；未导出时 tauri 走 TTY 提示，非交互必挂
  if [ "$PWD_EXPORTED" -eq 1 ]; then PASS "TAURI_SIGNING_PRIVATE_KEY_PASSWORD 已导出"; else
    FAIL "pwd-env" "TAURI_SIGNING_PRIVATE_KEY_PASSWORD 未导出——实证报错: incorrect updater private key password: Device not configured (os error 6)"; fi
  # 7 本仓库密钥为空密码加密：非空密码即解密必败
  if [ "$PWD_EXPORTED" -eq 1 ] && [ -n "$PWD_VALUE" ]; then
    FAIL "pwd-nonempty" "PASSWORD 为非空串——实证报错: Wrong password for that key（密钥为空密码加密，应 export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=\"\"）"
  elif [ "$PWD_EXPORTED" -eq 1 ]; then
    PASS "PASSWORD 为空串（与空密码加密密钥匹配）"
  fi
}

make_fixture_key() {
  python3 - "$1" <<'PYEOF'
import base64, sys
inner = b"untrusted comment: rsign encrypted secret key\nRWZBSUtFS0VZREVNTw==\n"
open(sys.argv[1], 'w').write(base64.b64encode(inner).decode())
PYEOF
}

self_test() {
  local tmp; tmp="$(mktemp -d)"
  local good="$tmp/good.key" bad64="$tmp/bad64.key" badhead="$tmp/badhead.key"
  make_fixture_key "$good"
  cp "$good" "$bad64"; printf '%%' >> "$bad64"
  python3 - "$badhead" <<'PYEOF'
import base64, sys
open(sys.argv[1], 'w').write(base64.b64encode(b"untrusted comment: other thing\nQQ==\n").decode())
PYEOF
  local script; script="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
  local rc=0
  expect() { # $1=场景名 $2=期望rc(0绿1红) 其后为 env 赋值
    local name="$1" want="$2"; shift 2
    ( eval "export $*"; bash "$script" --check-fixture >/dev/null 2>&1 )
    local got=$?
    if [ "$got" -eq "$want" ]; then echo "SELFTEST-PASS $name (rc=$got)"
    else echo "SELFTEST-FAIL $name 期望rc=$want 实际rc=$got"; rc=1; fi
  }
  expect "缺PATH变量"                 1 "KPE=" "KE=" "PE=0" "PV="
  expect "文件不存在"                  1 "KPE=$tmp/none.key" "KE=" "PE=0" "PV="
  expect "密钥带尾部杂字符(CI事故形态)" 1 "KPE=$bad64" "KE=$(cat $bad64)" "PE=1" "PV="
  expect "密钥头语义不符"               1 "KPE=$badhead" "KE=$(cat $badhead)" "PE=1" "PV="
  expect "KEY未导出"                  1 "KPE=$good" "KE=" "PE=1" "PV="
  expect "KEY与文件不一致"              1 "KPE=$good" "KE=disaster" "PE=1" "PV="
  expect "PASSWORD未导出"             1 "KPE=$good" "KE=$(cat $good)" "PE=0" "PV="
  expect "PASSWORD非空"               1 "KPE=$good" "KE=$(cat $good)" "PE=1" "PV=x"
  expect "全对=绿"                    0 "KPE=$good" "KE=$(cat $good)" "PE=1" "PV="
  rm -rf "$tmp"
  if [ "$rc" -eq 0 ]; then echo "self-test: 9/9 场景红绿符合预期"; else
    echo "self-test: 存在假绿/假红，脚本不可信" >&2; fi
  return "$rc"
}

check_fixture_mode() { # self-test 子进程：从注入变量取值（复用同一套检查）
  KEY_PATH_ENV="${KPE:-}"
  KEY_ENV="${KE:-}"
  PWD_EXPORTED="${PE:-0}"
  PWD_VALUE="${PV:-}"
}

case "${1:-}" in
  --self-test) self_test ;;
  --check-fixture) check_fixture_mode; run_checks; exit $(( FAILS > 0 ? 1 : 0 )) ;;
  *) gather_from_env; run_checks
     if [ "$FAILS" -gt 0 ]; then
       echo "预检未过：共 $FAILS 项红。按 FAIL 行提示修正后重跑；对照表见 docs/ops/updater-release.md。" >&2
       exit 1
     fi
     echo "预检全绿：本地签名环境齐备，可执行签名构建/打 tag。" ;;
esac
