#!/usr/bin/env bash
# 发布门禁：打 client-v<version> tag 前的全部机械校验 + 输出明确 tag 命令（0.1.1 发布事故后加入）
# 用法：bash scripts/check/release.sh <version> [--create]
#   默认只校验并打印命令，不创建 tag；--create 才在本地打 annotated tag（仍不 push，push 永远人工）
# 校验：semver / 三处版本一致且等于参数 / 工作树干净 / tag 本地+远端均不存在
# 分层：本脚本不含 make check 与分支校验（那是 make release-check 的职责），见 docs/release-gates.md
# 测试钩子：CHECK_ROOT 覆盖仓库根；RELEASE_SKIP_REMOTE=1 跳过远端 tag 查询（离线夹具用）
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${CHECK_ROOT:-$(cd "$SCRIPT_DIR/../.." && pwd)}"

fail() { echo "release-check: FAIL $*" >&2; exit 1; }

usage() {
  echo "用法：bash scripts/check/release.sh <version> [--create]" >&2
  echo "  <version>  semver，如 0.2.0 / 1.0.0-rc.1" >&2
  echo "  --create   校验通过后在本地创建 tag（默认只打印命令，不创建，避免误发布）" >&2
}

VERSION=""
CREATE=0
for arg in "$@"; do
  case "$arg" in
    --create) CREATE=1 ;;
    --*) usage; fail "未知参数 $arg" ;;
    *)
      if [ -n "$VERSION" ]; then usage; fail "版本号参数只允许一个（已收到 $VERSION）"; fi
      VERSION="$arg"
      ;;
  esac
done
[ -n "$VERSION" ] || { usage; fail "缺少版本号参数"; }

# 1. semver：MAJOR.MINOR.PATCH[-prerelease]；不接受 build metadata（+meta 与 tag 名有歧义）
printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$' \
  || fail "$VERSION 不是 semver（期望形如 0.2.0 / 1.0.0-rc.1）"

# 2. 三处版本一致且等于目标版本（复用 version.sh，保留其全部失败输出）
CHECK_ROOT="$ROOT" bash "$SCRIPT_DIR/version.sh" "$VERSION" \
  || fail "版本门禁未过（三处需一致且等于 $VERSION）"

# 3. 工作树干净：脏树打 tag = 发布内容不可复现
[ -n "$(git -C "$ROOT" status --porcelain)" ] && fail "工作树不干净（git status --porcelain 非空），先提交或清理"

# 4. tag 不存在：本地硬失败；远端可达时一并硬失败，不可达时显式 WARN（不静默吞错）
TAG="client-v$VERSION"
git -C "$ROOT" rev-parse -q --verify "refs/tags/$TAG" >/dev/null \
  && fail "本地已存在 tag $TAG"

if [ "${RELEASE_SKIP_REMOTE:-0}" != "1" ]; then
  if remote_ref="$(git -C "$ROOT" ls-remote --tags origin "refs/tags/$TAG" 2>&1)"; then
    [ -z "$remote_ref" ] || fail "远端 origin 已存在 tag $TAG"
    echo "release-check: 远端确认无 $TAG"
  else
    echo "release-check: WARN 查不到远端 tag（$(printf '%s' "$remote_ref" | head -n1)），仅完成本地校验" >&2
  fi
fi

COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)" || fail "无法读取 HEAD"

echo "release-check: PASS 门禁通过，HEAD=$COMMIT 目标 tag=$TAG"
echo "---- 发布命令（默认不执行；确认无误后加 --create 由本脚本打 tag）----"
echo "git -C '$ROOT' tag -a $TAG -m 'p2p-console $VERSION'"
echo "git -C '$ROOT' push origin $TAG"

if [ "$CREATE" -eq 1 ]; then
  git -C "$ROOT" tag -a "$TAG" -m "p2p-console $VERSION" || fail "tag 创建失败（$TAG）"
  echo "release-check: 已创建本地 tag $TAG（未 push；push 由人工执行）"
fi
