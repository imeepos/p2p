#!/usr/bin/env bash
# make-latest-json 自测（gate-tests）：合成产物树驱动 updater 清单生成红绿双向，
# 防清单生成器退化为假绿（2026-09-06 实锤：Tauri v2 产物形态与脚本 v1 假设
# 不符，linux 只有裸 .AppImage+.AppImage.sig，发布时必失败）。
#   绿：v2 形态四平台齐全 PASS 且 latest.json 平台/文件名逐一断言；
#       v1 tar.gz 形态兼容兜底仍 PASS；
#   红：缺任一 .sig（linux/windows/macos）必须失败——宁可发布失败不出残缺清单。
# 断言：退出码 + 输出标记 + latest.json 内容三条件；任何用例红则整体 exit 1。
set -u
set -o pipefail

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MJS="$CHECK_DIR/../../apps/gui/scripts/release/make-latest-json.mjs"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

pass=0
fail=0

t() { # t <名称> <期望退出码> <输出须含> <命令...>
  local name="$1" want_rc="$2" want_out="$3" out rc
  shift 3
    out="$($@ 2>&1)"; rc=$?
  if [ "$rc" -eq "$want_rc" ] && printf '%s' "$out" | grep -q "$want_out"; then
    pass=$((pass + 1)); echo "  ok   $name"
  else
    fail=$((fail + 1))
    echo "  FAIL ${name}（rc=${rc} 期望 ${want_rc}，输出应含 '${want_out}'）" >&2
    printf '%s\n' "$out" | sed 's/^/    | /' >&2
  fi
}

command -v node >/dev/null 2>&1 || { echo "make-latest-json 自测：SKIP（无 node）"; exit 0; }

# 合成产物树：$2=v2 用裸 .AppImage+.sig，$1 之外参数忽略；macOS 双架构同名件
make_tree() { # make_tree <dir> <v2|v1>
  mkdir -p "$1/artifacts/p2p-console-macos-latest" \
           "$1/artifacts/p2p-console-macos-15-intel" \
           "$1/artifacts/p2p-console-linux" \
           "$1/artifacts/p2p-console-windows"
  for arch in macos-latest macos-15-intel; do
    printf 'bin-%s' "$arch" > "$1/artifacts/p2p-console-$arch/p2p-console.app.tar.gz"
    printf 'sig-%s' "$arch" > "$1/artifacts/p2p-console-$arch/p2p-console.app.tar.gz.sig"
  done
  if [ "$2" = "v2" ]; then
    printf 'appimage-bin' > "$1/artifacts/p2p-console-linux/p2p-console_0.1.6_amd64.AppImage"
    printf 'appimage-sig' > "$1/artifacts/p2p-console-linux/p2p-console_0.1.6_amd64.AppImage.sig"
  else
    printf 'appimage-bin' > "$1/artifacts/p2p-console-linux/p2p-console_0.1.6_amd64.AppImage.tar.gz"
    printf 'appimage-sig' > "$1/artifacts/p2p-console-linux/p2p-console_0.1.6_amd64.AppImage.tar.gz.sig"
  fi
  printf 'nsis-bin' > "$1/artifacts/p2p-console-windows/p2p-console_0.1.6_x64-setup.exe"
  printf 'nsis-sig' > "$1/artifacts/p2p-console-windows/p2p-console_0.1.6_x64-setup.exe.sig"
}

echo "== v2 形态主路径（Tauri v2 实证形态） =="
V2="$WORK/v2"; make_tree "$V2" v2
t "v2 四平台齐全通过" 0 "latest-json: PASS" \
  node "$MJS" --artifacts "$V2/artifacts" --tag client-v0.1.6 --repo imeepos/p2p
LJ="$V2/artifacts/latest.json"
t "四平台键齐全" 0 "windows-x86_64" grep -o "windows-x86_64" "$LJ"
t "平台键 darwin-aarch64" 0 "darwin-aarch64" grep -o "darwin-aarch64" "$LJ"
t "平台键 darwin-x86_64" 0 "darwin-x86_64" grep -o "darwin-x86_64" "$LJ"
t "linux 选中裸 AppImage（v2 优先）" 0 "amd64.AppImage" grep -o "amd64.AppImage" "$LJ"
t "macos 改名带架构后缀" 0 "p2p-console-aarch64.app.tar.gz" grep -o "p2p-console-aarch64.app.tar.gz" "$LJ"

echo "== v1 tar.gz 兼容兜底 =="
V1="$WORK/v1"; make_tree "$V1" v1

t "v1 tar.gz 形态仍通过" 0 "latest-json: PASS" \
  node "$MJS" --artifacts "$V1/artifacts" --tag client-v0.1.6 --repo imeepos/p2p
t "v1 linux 选中 tar.gz" 0 "amd64.AppImage.tar.gz" grep -o "amd64.AppImage.tar.gz" "$V1/artifacts/latest.json"

echo "== 红路径：缺任一 .sig 必失败 =="
R1="$WORK/nolinuxsig"; make_tree "$R1" v2
rm "$R1/artifacts/p2p-console-linux/p2p-console_0.1.6_amd64.AppImage.sig"
t "缺 linux .sig 必红" 1 "签名缺失或重复" \
  node "$MJS" --artifacts "$R1/artifacts" --tag client-v0.1.6 --repo imeepos/p2p
R2="$WORK/nowinsig"; make_tree "$R2" v2
rm "$R2/artifacts/p2p-console-windows/p2p-console_0.1.6_x64-setup.exe.sig"
t "缺 windows .sig 必红" 1 "签名缺失或重复" \
  node "$MJS" --artifacts "$R2/artifacts" --tag client-v0.1.6 --repo imeepos/p2p
R3="$WORK/nomacsig"; make_tree "$R3" v2
rm "$R3/artifacts/p2p-console-macos-15-intel/p2p-console.app.tar.gz.sig"
t "缺 macos intel .sig 必红" 1 "签名缺失或重复" \
  node "$MJS" --artifacts "$R3/artifacts" --tag client-v0.1.6 --repo imeepos/p2p

echo "== 结果：$pass 通过 / $fail 失败 =="
if [ "$fail" -ne 0 ]; then
  echo "make-latest-json 自测：FAIL" >&2
  exit 1
fi
echo "make-latest-json 自测：PASS"
