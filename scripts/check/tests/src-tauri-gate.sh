#!/usr/bin/env bash
# src-tauri 门禁覆盖自测（gate-tests）：验证 fmt.sh/clippy.sh 的 src-tauri 段，
# 防门禁实现退化为假绿（2026-09-05 盲区事故：src-tauri 被根 workspace exclude
# 后根 fmt/clippy 全扫不到，chat.rs 漂移与 group_contract 警告随 PR 漏网）。
#   fmt 段：CHECK_ROOT 夹具 crate 植入漂移必红、清除后回绿（rustfmt 纯解析，双平台真跑）
#   clippy 段：GUI_TAURI_SKIP=1 逃生口可观测；可编译环境（macOS/有 webkit2gtk-4.1）
#     植入 clippy 警告必红、清除后回绿；非 macOS 缺系统库（CI ubuntu）按
#     gui-tauri.sh 同口径断言 SKIP 标记（显式可观测，不假绿）
# 断言：退出码 + 输出标记双条件；任何用例红则整体 exit 1（机械可判）
set -u
set -o pipefail

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

# 夹具：根 workspace（含一个零依赖成员，cargo fmt/clippy 有目标可跑）+
# 独立 src-tauri 夹具 crate（零依赖，秒级编译）
make_fixture() { # make_fixture <dir>
  mkdir -p "$1/apps/gui/src-tauri/src" "$1/apps/gui/src-tauri/tests" "$1/crates/root-fixture/src"
  printf '[workspace]\nresolver = "2"\nmembers = ["crates/root-fixture"]\nexclude = ["apps/gui/src-tauri"]\n' > "$1/Cargo.toml"
  cat > "$1/crates/root-fixture/Cargo.toml" <<'EOF'
[package]
name = "root-fixture"
version = "0.0.0"
edition = "2021"
EOF
  printf 'pub fn clean() {}\n' > "$1/crates/root-fixture/src/lib.rs"
  cat > "$1/apps/gui/src-tauri/Cargo.toml" <<'EOF'
[package]
name = "gate-fixture"
version = "0.0.0"
edition = "2021"

[workspace]
EOF
  printf 'pub fn clean() {}\n' > "$1/apps/gui/src-tauri/src/lib.rs"
  printf '#[test]\nfn placeholder() {}\n' > "$1/apps/gui/src-tauri/tests/group_contract.rs"
}

plant_fmt_drift() { # plant_fmt_drift <dir>
  printf 'pub fn drifted( ) -> u8 { 1 }\n' > "$1/apps/gui/src-tauri/src/lib.rs"
}

plant_clippy_warn() { # plant_clippy_warn <dir>
  cat > "$1/apps/gui/src-tauri/tests/group_contract.rs" <<'EOF'
#[test]
fn needless_return_fixture() {
    fn f() -> u8 { return 1 }
    assert_eq!(f(), 1);
}
EOF
}

echo "== fmt.sh src-tauri 段 =="
F="$WORK/fmt"; make_fixture "$F"
t "干净夹具通过（根空 workspace + src-tauri 干净）" 0 "fmt: PASS" env CHECK_ROOT="$F" bash "$CHECK_DIR/fmt.sh"
plant_fmt_drift "$F"
t "src-tauri 植入漂移必红" 1 "src-tauri 存在格式漂移" env CHECK_ROOT="$F" bash "$CHECK_DIR/fmt.sh"
printf 'pub fn clean() {}\n' > "$F/apps/gui/src-tauri/src/lib.rs"
t "漂移清除后回绿" 0 "fmt: src-tauri 段 PASS" env CHECK_ROOT="$F" bash "$CHECK_DIR/fmt.sh"

echo "== clippy.sh src-tauri 段 =="
C="$WORK/clippy"; make_fixture "$C"
t "GUI_TAURI_SKIP=1 逃生口可观测" 0 "SKIP src-tauri 段" env CHECK_ROOT="$C" GUI_TAURI_SKIP=1 bash "$CHECK_DIR/clippy.sh"
plant_clippy_warn "$C"
if [ "$(uname -s)" = "Darwin" ] || pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  t "src-tauri 植入 clippy 警告必红" 1 "src-tauri 有警告" env CHECK_ROOT="$C" bash "$CHECK_DIR/clippy.sh"
  printf '#[test]\nfn placeholder() {}\n' > "$C/apps/gui/src-tauri/tests/group_contract.rs"
  t "警告清除后回绿" 0 "clippy: PASS" env CHECK_ROOT="$C" bash "$CHECK_DIR/clippy.sh"
else
  t "非 macOS 缺系统库按口径 SKIP（显式可观测）" 0 "SKIP src-tauri 段" env CHECK_ROOT="$C" bash "$CHECK_DIR/clippy.sh"
fi

echo "== 结果：$pass 通过 / $fail 失败 =="
if [ "$fail" -ne 0 ]; then
  echo "src-tauri-gate 自测：FAIL" >&2
  exit 1
fi
echo "src-tauri-gate 自测：PASS"
