#!/usr/bin/env bash
# 受影响域判定（make check-fast 的裁剪依据，纯只读不改任何文件）：
#   git 变更集（基线 diff + 未提交 + untracked）→ 域标志 + workspace crate 反向依赖闭包。
#   stdout 只输出 KEY=VALUE 行（供 fast.sh eval），人读信息一律走 stderr。
# 域模型：
#   FULL_ALL         门禁实现自身变更（Makefile/scripts/githooks/keys/rust-toolchain）→ 全量
#   FULL_RUST        根 Cargo.toml/Cargo.lock 变更（依赖图漂移）→ 根 workspace 全量
#   DOMAIN_GUI       apps/gui（除 src-tauri）或 pnpm-lock.yaml
#   DOMAIN_TAURI     apps/gui/src-tauri
#   AFFECTED_CRATES  变更 crate 及其全部 workspace 依赖者（normal+dev+build 边）
# 基线：AFFECTED_BASE 显式指定 > origin/main > main > HEAD；基线 diff 取 merge-base，
#   并叠加工作树未提交变更与 untracked。干净主树 → 全零（诚实输出零域，不假装验证过）。
# 保守回退：crate 路径非法 / 闭包计算失败（crate 删改名、cargo 异常）→ FULL_RUST=1，
#   宁可多跑不可漏跑；任何回退都在 stderr 留观测信号，禁止静默缩小范围。
# 约定：crates/<目录名> 与包名一致（与现仓库一致）；不一致时闭包按失败回退 FULL_RUST。
# 测试钩子：CHECK_ROOT 覆盖仓库根（scripts/check/tests/affected-fast.sh 夹具驱动）
# 自测：scripts/check/tests/affected-fast.sh（合成 git+cargo workspace，红绿双向）
set -u
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"

ROOT="${CHECK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT" || exit 1

err() { echo "affected: $1" >&2; }

# --- 基线引用：显式 env > origin/main > main > HEAD ---
base_ref() {
  if [ -n "${AFFECTED_BASE:-}" ]; then
    echo "$AFFECTED_BASE"
    return
  fi
  if git rev-parse -q --verify origin/main >/dev/null 2>&1; then
    echo origin/main
  elif git rev-parse -q --verify main >/dev/null 2>&1; then
    echo main
  else
    echo HEAD
  fi
}
BASE="$(base_ref)"
BASE_COMMIT="$(git merge-base "$BASE" HEAD 2>/dev/null || echo "$BASE")"

# --- 变更集：基线 diff + 未提交（staged/unstaged）+ untracked，去重 ---
CHANGED="$(
  {
    git diff --name-only "$BASE_COMMIT" -- 2>/dev/null
    git diff --name-only HEAD --
    git ls-files --others --exclude-standard
  } | sort -u
)"

FULL_ALL=0
FULL_RUST=0
DOMAIN_GUI=0
DOMAIN_TAURI=0
CRATES_RAW=""

sanitize_crate() { # crate 名白名单（进 eval 输出，必须不可注入）；不合法返回非零
  case "$1" in
    *[!A-Za-z0-9_-]* | "") return 1 ;;
    *) return 0 ;;
  esac
}

while IFS= read -r f; do
  [ -z "$f" ] && continue
  case "$f" in
    crates/*)
      c="${f#crates/}"
      c="${c%%/*}"
      if sanitize_crate "$c"; then
        CRATES_RAW="$CRATES_RAW $c"
      else
        err "crate 路径不合法: $f → 回退 FULL_RUST"
        FULL_RUST=1
      fi
      ;;
    Cargo.toml | Cargo.lock) FULL_RUST=1 ;;
    apps/gui/src-tauri/*) DOMAIN_TAURI=1 ;;
    apps/gui/*) DOMAIN_GUI=1 ;;
    pnpm-lock.yaml) DOMAIN_GUI=1 ;;
    rust-toolchain.toml | rust-toolchain) FULL_ALL=1 ;;
    Makefile | scripts/* | githooks/* | keys/* | .cargo/*) FULL_ALL=1 ;;
  esac
done <<EOF
$CHANGED
EOF

# --- 反向依赖闭包：cargo tree 倒排，取路径依赖（即 workspace 成员）名集合 ---
closure_of() { # $1=crate 名；失败输出空
  # 两个 cargo tree 陷阱（2026-09-09 夹具自测实锤）：
  #   1. 必须用位置参数包名而非 -p——-p 会把解析图限定到该包自身子树，倒排找不到依赖者
  #   2. 位置包名必须在 -e 之前——"-e normal,dev,build fx-a" 会报 unexpected argument
  # 名字提取用 grep -oE 而非行首锚定 sed——倒排树带 └── 等树形前缀，BSD sed
  # 的 ^[[:space:]]* 吃不掉它们（同日实锤），-oE 直接抓「名字 v版本」片段
  cargo tree -i "$1" -e normal,dev,build 2>/dev/null |
    grep '(/' |
    grep -oE '[A-Za-z0-9_-]+ v[0-9]' |
    sed -E 's/ v[0-9]$//' |
    sort -u
}

CRATES_RAW="$(printf '%s\n' $CRATES_RAW | sort -u)"
AFFECTED_CRATES=""
if [ "$FULL_RUST" = 1 ]; then
  AFFECTED_CRATES="" # fast.sh 按 --workspace 全量处理
else
  # workspace 成员全集：倒排闭包会混入 workspace 之外的 path 依赖者
  # （如被 exclude 的 apps/acp-agent 依赖 crates 内库），全量 check 不覆盖它们，
  # 闭包也必须排除——否则 check-fast 与全量口径分裂（2026-09-09 p2p-log 实测）
  MEMBERS="$(cargo tree --workspace --depth 0 -e normal 2>/dev/null |
    grep -oE '[A-Za-z0-9_-]+ v[0-9]' | sed -E 's/ v[0-9]$//' | sort -u)"
  if [ -z "$MEMBERS" ]; then
    err "workspace 成员枚举失败 → 回退 FULL_RUST"
    FULL_RUST=1
    AFFECTED_CRATES=""
  fi
  for c in $CRATES_RAW; do
    sub="$(closure_of "$c" | grep -Fx -f <(printf '%s\n' "$MEMBERS") 2>/dev/null)"
    if [ -z "$sub" ]; then
      err "闭包计算失败（$c）→ 回退 FULL_RUST"
      FULL_RUST=1
      AFFECTED_CRATES=""
      break
    fi
    AFFECTED_CRATES="$AFFECTED_CRATES $sub"
  done
  if [ "$FULL_RUST" = 0 ]; then
    AFFECTED_CRATES="$(printf '%s\n' $AFFECTED_CRATES | sort -u | tr '\n' ' ')"
    AFFECTED_CRATES="${AFFECTED_CRATES% }"
    AFFECTED_CRATES="${AFFECTED_CRATES# }"
  fi
fi

if [ "$FULL_ALL" = 1 ]; then
  FULL_RUST=1
  DOMAIN_GUI=1
  DOMAIN_TAURI=1
  AFFECTED_CRATES=""
fi

# --- 机器可读输出（fast.sh eval 用；值均已约束，无注入面） ---
echo "FULL_ALL=$FULL_ALL"
echo "FULL_RUST=$FULL_RUST"
echo "DOMAIN_GUI=$DOMAIN_GUI"
echo "DOMAIN_TAURI=$DOMAIN_TAURI"
echo "AFFECTED_CRATES=\"$AFFECTED_CRATES\""
echo "AFFECTED_BASE=$BASE"

n_changed="$(printf '%s\n' "$CHANGED" | grep -c . || true)"
err "基线=$BASE 变更文件=$n_changed crates=[$AFFECTED_CRATES] full_rust=$FULL_RUST gui=$DOMAIN_GUI tauri=$DOMAIN_TAURI full_all=$FULL_ALL"
