#!/usr/bin/env bash
# mock 泄漏护栏自测（gate-tests）：防两道护栏自身退化为假绿。
# - gui-dist-scan.sh：成功/失败路径走临时夹具（干净/内容特征/文件名特征/缺目录）
# - vite 构建期断言：真实红路径探测，VITE_MOCK_IPC=1 进 build 必须在
#   configResolved 即被拦（bundle 未开始，秒级）。依赖 apps/gui 的
#   node_modules（CI 在 make check 前已 pnpm install）。
# 断言：退出码 + 输出标记双条件；任何用例红则整体 exit 1（机械可判）。
set -u
set -o pipefail

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$CHECK_DIR/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

pass=0
fail=0

t() { # t <名称> <期望退出码> <输出须含> <命令...>
  local name="$1" want_rc="$2" want_out="$3" out rc
  shift 3
  out="$("$@" 2>&1)"; rc=$?
  if [ "$rc" -eq "$want_rc" ] && printf '%s' "$out" | grep -q "$want_out"; then
    pass=$((pass + 1)); echo "  ok   $name"
  else
    fail=$((fail + 1))
    echo "  FAIL ${name}（rc=${rc} 期望 ${want_rc}，输出应含 '${want_out}'）" >&2
    printf '%s\n' "$out" | sed 's/^/    | /' >&2
  fi
}

echo "== gui-dist-scan.sh 夹具 =="

# 绿：干净产物（含正常 js 与子目录，无任何 mock 特征）
mkdir -p "$WORK/clean/assets"
echo 'console.log("app entry")' > "$WORK/clean/assets/index.js"
t "干净产物通过" 0 "gui-dist-scan: PASS" bash "$CHECK_DIR/gui-dist-scan.sh" "$WORK/clean"

# 红：内容特征（chunk 引用串/标识符）
mkdir -p "$WORK/leak/assets"
echo 'import("./mock-ipc-BgtC7ZgN.js")' > "$WORK/leak/assets/index.js"
t "内容特征拦截" 1 "mock 特征" bash "$CHECK_DIR/gui-dist-scan.sh" "$WORK/leak"

# 红：文件名特征（泄漏 chunk 以 mock-*.js 落盘）
mkdir -p "$WORK/chunk/assets"
echo 'export {};' > "$WORK/chunk/assets/mock-update-CoQmjkIV.js"
t "文件名特征拦截" 1 "mock 特征" bash "$CHECK_DIR/gui-dist-scan.sh" "$WORK/chunk"

# 红：产物目录缺失（显式可观测，不静默跳过）
t "产物目录缺失拦截" 1 "产物目录不存在" bash "$CHECK_DIR/gui-dist-scan.sh" "$WORK/missing"

echo "== vite 构建期断言（真实红路径） =="

mock_build_probe() { (cd "$ROOT/apps/gui" && env VITE_MOCK_IPC=1 pnpm exec vite build); }
t "VITE_MOCK_IPC=1 构建必红" 1 "no-mock-in-build" mock_build_probe

echo "== 结果：$pass 通过 / $fail 失败 =="
if [ "$fail" -ne 0 ]; then
  echo "mock-ipc-guards 自测：FAIL" >&2
  exit 1
fi
echo "mock-ipc-guards 自测：PASS"
