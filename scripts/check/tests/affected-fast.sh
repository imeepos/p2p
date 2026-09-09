#!/usr/bin/env bash
# affected.sh / fast.sh 自测（gate-tests）：合成 git+cargo workspace 夹具，红绿双向，
# 防快门禁退化为假绿。核心保护目标：affected.sh 绝不漏报受影响域（漏=假绿），
# 保守回退必须生效；fast.sh 的 SKIP/PASS/FAIL 编排与退出码正确。
#   绿：leaf crate 改动 → 闭包含其依赖者；gui/tauri 路径各自成域；
#       fast.sh 端到端 PASS 且 SKIP 无变更域
#   红：脚本类变更 → FULL_ALL；根 Cargo.lock → FULL_RUST；
#       夹具 crate 清单损坏 → 保守回退 FULL_RUST；夹具门禁失败 → fast.sh 退出 1
# 夹具：mktemp 内 git init + 双 crate（b 依赖 a，零外部依赖，离线可编译），
#   scripts/check 放真实 affected.sh/fast.sh 拷贝 + 桩门禁（即时 PASS/可控 FAIL）。
set -u
set -o pipefail

export PATH="$HOME/.cargo/bin:$PATH"

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REAL_CHECK="$CHECK_DIR"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

pass=0
fail=0

ok() { pass=$((pass + 1)); echo "  ok   $1"; }
ng() { fail=$((fail + 1)); echo "  FAIL $1" >&2; }

assert_field() { # assert_field <说明> <输出> <期望完整行>
  local name="$1" out="$2" want="$3"
  if printf '%s\n' "$out" | grep -qx "$want"; then
    ok "$name"
  else
    ng "$name（期望行: $want；实际: $(printf '%s\n' "$out" | grep -E '^(FULL|DOMAIN|AFFECTED_CRATES)' | tr '\n' ' ')）"
  fi
}

assert_crate_in() { # assert_crate_in <说明> <输出> <crate 名>
  local name="$1" out="$2" crate="$3"
  local line
  line="$(printf '%s\n' "$out" | grep '^AFFECTED_CRATES=' || true)"
  if printf '%s' "$line" | grep -qE "[\" ]${crate}[ \"]"; then
    ok "$name"
  else
    ng "$name（$crate 不在 $line）"
  fi
}

assert_crate_not_in() { # assert_crate_not_in <说明> <输出> <crate 名>
  local name="$1" out="$2" crate="$3"
  local line
  line="$(printf '%s\n' "$out" | grep '^AFFECTED_CRATES=' || true)"
  if printf '%s' "$line" | grep -qE "[\" ]${crate}[ \"]"; then
    ng "$name（$crate 不应出现在 $line）"
  else
    ok "$name"
  fi
}

# --- 夹具构造 ---
mk_fixture() { # 输出夹具根；桩门禁默认全 PASS
  # 注意：crate 目录名必须与包名一致（affected.sh 约定，与真实仓库一致）
  # fx-c 为 exclude 形态回归夹具：workspace 之外的 path 依赖者（对齐 apps/acp-agent）
  local fx="$1"
  mkdir -p "$fx/crates/fx-a/src" "$fx/crates/fx-b/src" "$fx/crates/fx-c/src" "$fx/scripts/check/tests" "$fx/apps/gui/src-tauri/src"
  cat >"$fx/Cargo.toml" <<'TOML'
[workspace]
resolver = "2"
members = ["crates/fx-a", "crates/fx-b"]
exclude = ["crates/fx-c"]
TOML
  cat >"$fx/crates/fx-a/Cargo.toml" <<'TOML'
[package]
name = "fx-a"
version = "0.1.0"
edition = "2021"
TOML
  echo 'pub fn a() {}' >"$fx/crates/fx-a/src/lib.rs"
  cat >"$fx/crates/fx-b/Cargo.toml" <<'TOML'
[package]
name = "fx-b"
version = "0.1.0"
edition = "2021"

[dependencies]
fx-a = { path = "../fx-a" }
TOML
  echo 'pub fn b() { fx_a::a(); }' >"$fx/crates/fx-b/src/lib.rs"
  cat >"$fx/crates/fx-c/Cargo.toml" <<'TOML'
[package]
name = "fx-c"
version = "0.1.0"
edition = "2021"

[dependencies]
fx-a = { path = "../fx-a" }
TOML
  echo 'pub fn c() { fx_a::a(); }' >"$fx/crates/fx-c/src/lib.rs"
  cp "$REAL_CHECK/affected.sh" "$REAL_CHECK/fast.sh" "$fx/scripts/check/"
  local g
  for g in tests/release-gates.sh tests/panic-hygiene.sh tests/cli-parity.sh \
    tests/mock-ipc-guards.sh tests/src-tauri-gate.sh tests/make-latest-json.sh \
    version.sh fmt.sh line-limit.sh panic-hygiene.sh cli-parity.sh ai-docs-sync.sh; do
    printf '#!/usr/bin/env bash\necho stub %s\nexit 0\n' "$g" >"$fx/scripts/check/$g"
  done
  git -C "$fx" init -q
  # 先生成 Cargo.lock 再入库：对齐真实仓库 lock 已跟踪的事实；否则首轮 cargo tree
  # 生成的未跟踪 lock 会被下一轮 affected.sh 误判为依赖图变更（FULL_RUST 假红）
  (cd "$fx" && cargo metadata --format-version 1 >/dev/null 2>&1)
  git -C "$fx" -c user.email=t@t -c user.name=t add -A
  git -C "$fx" -c user.email=t@t -c user.name=t commit -qm base
  echo "$fx"
}

aff() { # aff <夹具根>：跑 affected.sh，stdout=KEY=VALUE 输出
  CHECK_ROOT="$1" bash "$1/scripts/check/affected.sh" 2>/dev/null
}

change() { # change <夹具根> <相对路径> <新内容>：未提交变更
  mkdir -p "$(dirname "$1/$2")"
  printf '%s' "$3" >"$1/$2"
}

F="$(mk_fixture "$WORK/fx")"

# 1 干净树 → 全零
out="$(aff "$F")"
assert_field "干净树 FULL_ALL=0" "$out" "FULL_ALL=0"
assert_field "干净树 AFFECTED_CRATES 空" "$out" 'AFFECTED_CRATES=""'

# 2 leaf crate 改动 → 闭包含依赖者，域全零
change "$F" crates/fx-a/src/lib.rs 'pub fn a() { println!("x"); }'
out="$(aff "$F")"
assert_crate_in "leaf 改动闭包含自身 fx-a" "$out" fx-a
assert_crate_in "leaf 改动闭包含依赖者 fx-b" "$out" fx-b
assert_crate_not_in "exclude 的 fx-c 不进闭包（全量 check 也不覆盖它）" "$out" fx-c
assert_field "leaf 改动 GUI=0" "$out" "DOMAIN_GUI=0"
assert_field "leaf 改动 TAURI=0" "$out" "DOMAIN_TAURI=0"

# 3 gui / tauri / lock / scripts 各域
change "$F" apps/gui/src/main.tsx 'export {}'
out="$(aff "$F")"
assert_field "gui 改动 DOMAIN_GUI=1" "$out" "DOMAIN_GUI=1"
assert_field "gui 改动不影响 rust 闭包判定" "$out" "FULL_RUST=0"
change "$F" apps/gui/src-tauri/src/main.rs 'fn main() {}'
out="$(aff "$F")"
assert_field "tauri 改动 DOMAIN_TAURI=1" "$out" "DOMAIN_TAURI=1"
change "$F" Cargo.lock ''
out="$(aff "$F")"
assert_field "Cargo.lock → FULL_RUST=1" "$out" "FULL_RUST=1"
change "$F" scripts/check/nonsense.sh ''
out="$(aff "$F")"
assert_field "scripts 变更 → FULL_ALL=1" "$out" "FULL_ALL=1"
assert_field "FULL_ALL 强制 GUI=1" "$out" "DOMAIN_GUI=1"

# 4 crate 清单损坏 → 保守回退 FULL_RUST（防闭包失真静默缩小范围）
F2="$(mk_fixture "$WORK/fx2")"
change "$F2" crates/fx-a/src/lib.rs 'pub fn a2() {}'
change "$F2" crates/fx-b/Cargo.toml 'broken toml <<<'
out="$(aff "$F2")"
assert_field "清单损坏回退 FULL_RUST=1" "$out" "FULL_RUST=1"

# 5 fast.sh 端到端：leaf 改动 → 闭包 clippy/test 真跑 + gui/tauri SKIP + PASS
F3="$(mk_fixture "$WORK/fx3")"
change "$F3" crates/fx-a/src/lib.rs 'pub fn a() { println!("y"); }'
if CHECK_ROOT="$F3" bash "$F3/scripts/check/fast.sh" >/dev/null 2>&1; then
  ok "fast.sh 端到端退出 0"
else
  ng "fast.sh 端到端退出 0"
fi
fast_out="$(CHECK_ROOT="$F3" bash "$F3/scripts/check/fast.sh" 2>&1 || true)"
printf '%s\n' "$fast_out" | grep -q "SKIP gui-check" && ok "fast.sh SKIP gui-check" || ng "fast.sh SKIP gui-check"
printf '%s\n' "$fast_out" | grep -q "PASS test-closure" && ok "fast.sh 真跑 test-closure" || ng "fast.sh 真跑 test-closure"
printf '%s\n' "$fast_out" | grep -q "PASS clippy-closure" && ok "fast.sh 真跑 clippy-closure" || ng "fast.sh 真跑 clippy-closure"

# 6 fast.sh 红路径：桩门禁失败 → 退出 1 且报 FAIL
printf '#!/usr/bin/env bash\nexit 1\n' >"$F3/scripts/check/panic-hygiene.sh"
if CHECK_ROOT="$F3" bash "$F3/scripts/check/fast.sh" >/dev/null 2>&1; then
  ng "fast.sh 红路径应退出 1"
else
  ok "fast.sh 红路径退出 1"
fi
fast_red="$(CHECK_ROOT="$F3" bash "$F3/scripts/check/fast.sh" 2>&1 || true)"
printf '%s\n' "$fast_red" | grep -q "FAIL panic-hygiene" && ok "fast.sh 报 FAIL panic-hygiene" || ng "fast.sh 报 FAIL panic-hygiene"

echo "affected-fast: pass=$pass fail=$fail"
[ "$fail" -eq 0 ]
