#!/usr/bin/env bash
# 版本/发布门禁自测：临时夹具驱动 version.sh 与 release.sh 的成功/失败路径
# 用法：bash scripts/check/tests/release-gates.sh
# 断言：退出码 + 输出标记双条件；任何用例红则整体 exit 1（机械可判，不靠人眼）
set -u
set -o pipefail

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

pass=0
fail=0

t() { # t <名称> <期望退出码> <输出须含> <命令...>（环境变量经 env 前缀传入）
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

# 夹具：三处版本文件。Cargo.toml 故意带内联依赖 version = "2"，验证提取不误读
make_fixture() { # make_fixture <dir> <pkg版本> <conf版本> <cargo版本>
  mkdir -p "$1/apps/gui/src-tauri"
  printf '{\n  "name": "@p2p/gui",\n  "version": "%s"\n}\n' "$2" > "$1/apps/gui/package.json"
  printf '{\n  "productName": "p2p-console",\n  "version": "%s"\n}\n' "$3" > "$1/apps/gui/src-tauri/tauri.conf.json"
  printf '[package]\nname = "p2p-console"\nversion = "%s"\n\n[dependencies]\ntauri = { version = "2" }\n' "$4" > "$1/apps/gui/src-tauri/Cargo.toml"
}

make_git_fixture() { # make_git_fixture <dir> <版本>：三处同值 + 提交干净的 git 仓库
  make_fixture "$1" "$2" "$2" "$2"
  git -C "$1" init -q -b main
  git -C "$1" config user.name gate-test
  git -C "$1" config user.email gate-test@example.invalid
  git -C "$1" add -A
  git -C "$1" commit -qm "fixture $2"
}

echo "== version.sh =="

F="$WORK/consistent"; make_fixture "$F" 1.2.3 1.2.3 1.2.3
t "三处一致通过" 0 "PASS" env CHECK_ROOT="$F" bash "$CHECK_DIR/version.sh"
t "期望版本匹配通过" 0 "PASS 三处版本一致：1.2.3" env CHECK_ROOT="$F" bash "$CHECK_DIR/version.sh" 1.2.3
t "期望版本不匹配失败" 1 "期望版本 9.9.9" env CHECK_ROOT="$F" bash "$CHECK_DIR/version.sh" 9.9.9

G="$WORK/mismatch"; make_fixture "$G" 1.2.3 1.2.4 1.2.3
t "三处不一致失败" 1 "version-check: FAIL" env CHECK_ROOT="$G" bash "$CHECK_DIR/version.sh"

H="$WORK/missing"; mkdir -p "$H"
t "文件缺失失败" 1 "缺少文件" env CHECK_ROOT="$H" bash "$CHECK_DIR/version.sh"

t "真实仓库三处一致" 0 "PASS" bash "$CHECK_DIR/version.sh"

echo "== release.sh =="

R="$WORK/rel"; make_git_fixture "$R" 0.2.0
t "缺少版本号参数" 1 "缺少版本号" env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh"
t "未知参数拒绝" 1 "未知参数" env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.2.0 --bad
t "非 semver 拒绝" 1 "不是 semver" env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.2
t "版本不等于参数拒绝" 1 "版本门禁未过" env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.3.0

touch "$R/untracked.txt"
t "脏树拒绝" 1 "工作树不干净" env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.2.0
rm "$R/untracked.txt"

git -C "$R" tag -a client-v0.2.0 -m fixture
t "本地 tag 已存在拒绝" 1 "本地已存在 tag" env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.2.0
git -C "$R" tag -d client-v0.2.0 >/dev/null

# 远端 tag：裸仓库当 origin，推上去后删本地，验证远端查询能拦住重复发布
git -C "$R" init -q --bare "$WORK/origin.git"
git -C "$R" remote add origin "$WORK/origin.git"
git -C "$R" tag -a client-v0.2.0 -m fixture
git -C "$R" push -q origin client-v0.2.0
git -C "$R" tag -d client-v0.2.0 >/dev/null
t "远端 tag 已存在拒绝" 1 "远端 origin 已存在" env CHECK_ROOT="$R" bash "$CHECK_DIR/release.sh" 0.2.0

# 成功路径一：默认不创建 tag，只打印命令
out="$(env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.2.0 2>&1)"; rc=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "tag -a client-v0.2.0" \
   && [ -z "$(git -C "$R" tag -l)" ]; then
  pass=$((pass + 1)); echo "  ok   默认只打印命令不创建 tag"
else
  fail=$((fail + 1)); echo "  FAIL 默认只打印命令不创建 tag（rc=${rc}）" >&2
  printf '%s\n' "$out" | sed 's/^/    | /' >&2
fi

# 成功路径二：--create 创建 annotated tag 且指向 HEAD
out="$(env CHECK_ROOT="$R" RELEASE_SKIP_REMOTE=1 bash "$CHECK_DIR/release.sh" 0.2.0 --create 2>&1)"; rc=$?
tagged="$(git -C "$R" rev-parse -q --verify 'refs/tags/client-v0.2.0^{commit}')"
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "已创建本地 tag" \
   && [ -n "$tagged" ] && [ "$tagged" = "$(git -C "$R" rev-parse HEAD)" ]; then
  pass=$((pass + 1)); echo "  ok   --create 生成指向 HEAD 的 annotated tag"
else
  fail=$((fail + 1)); echo "  FAIL --create（rc=$rc tagged=${tagged:-无}）" >&2
  printf '%s\n' "$out" | sed 's/^/    | /' >&2
fi

# 远端不可达：显式 WARN 且不拦截（可观测，不静默）
R2="$WORK/rel-noremote"; make_git_fixture "$R2" 0.1.0
t "远端不可达 WARN 不拦" 0 "WARN 查不到远端" env CHECK_ROOT="$R2" bash "$CHECK_DIR/release.sh" 0.1.0

echo "== 结果：$pass 通过 / $fail 失败 =="
if [ "$fail" -ne 0 ]; then
  echo "release-gates 自测：FAIL" >&2
  exit 1
fi
echo "release-gates 自测：PASS"
