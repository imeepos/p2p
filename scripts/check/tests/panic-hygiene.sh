#!/usr/bin/env bash
# panic-hygiene 门禁自测：临时夹具驱动红/绿/豁免/保护路径（防门禁假绿）
# 断言：退出码 + 输出标记双条件；任何用例红则整体 exit 1（机械可判，不靠人眼）
set -uo pipefail

CHECK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$CHECK_DIR/panic-hygiene.sh"
REAL_EXEMPT="$CHECK_DIR/panic-hygiene-exempt.txt"
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
    echo "  FAIL ${name}（rc=$rc 期望 ${want_rc}，输出应含 '$want_out'）" >&2
    printf '%s\n' "$out" | sed 's/^/    | /' >&2
  fi
}

# 干净夹具：src 非测试路径零违规；#[cfg(test)] 模块与 tests/ 目录内的 unwrap 不计数
make_fixture() { # make_fixture <dir>
  mkdir -p "$1/crates/demo/src" "$1/crates/demo/tests"
  cat > "$1/crates/demo/src/lib.rs" <<'EOF'
pub fn parse_or_zero(s: &str) -> u64 {
    s.parse().unwrap_or(0)
}

pub fn guarded(s: &str) -> u64 {
    match s.parse::<u64>() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("parse failed: {err}");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn inner() {
        let x: Option<u8> = None;
        let _ = x.unwrap();
    }
}
EOF
  cat > "$1/crates/demo/src/cache_tests.rs" <<'EOF'
#[cfg(test)]
mod tests {
    #[test]
    fn unit() {
        let x: Option<u8> = None;
        let _ = x.expect("fixture");
    }
}
EOF
  printf 'pub fn it() {}\n' > "$1/crates/demo/tests/it.rs"
}

plant() { # plant <dir>：植入违规样例（独立文件，整文件删除即清除）
  printf 'pub fn bad(s: &str) -> u64 { s.parse().unwrap() }\npub fn boom() { panic!("no") }\n' \
    > "$1/crates/demo/src/bad.rs"
}

echo "== 绿路径：干净夹具（测试代码含 unwrap 不计） =="
G="$WORK/green"; make_fixture "$G"
t "干净夹具通过" 0 "panic-hygiene: PASS" env CHECK_ROOT="$G" bash "$GATE"
t "*_tests.rs 约定文件不计非测试路径" 0 "panic-hygiene: PASS" env CHECK_ROOT="$G" bash "$GATE"

echo "== 红路径：植入违规须红 =="
R="$WORK/red"; make_fixture "$R"; plant "$R"
t "植入 unwrap/panic 变红" 1 "panic-hygiene: FAIL" env CHECK_ROOT="$R" bash "$GATE"
t "违规定位到植入文件" 1 "src/bad.rs" env CHECK_ROOT="$R" bash "$GATE"

echo "== 清除后须绿 =="
rm "$R/crates/demo/src/bad.rs"
t "清除违规后回绿" 0 "panic-hygiene: PASS" env CHECK_ROOT="$R" bash "$GATE"

echo "== 豁免路径 =="
E="$WORK/exempt"; make_fixture "$E"
mkdir -p "$E/crates/p2p-itest/src"
printf 'pub fn boom() { panic!("fixture") }\n' > "$E/crates/p2p-itest/src/lib.rs"
t "真实豁免清单放行 p2p-itest" 0 "panic-hygiene: PASS" env CHECK_ROOT="$E" bash "$GATE"
printf 'demo 只豁免 demo 不含 p2p-itest\n' > "$WORK/manifest-noincl"
t "清单不含违规 crate 则红" 1 "panic-hygiene: FAIL" env CHECK_ROOT="$E" PANIC_HYGIENE_EXEMPT="$WORK/manifest-noincl" bash "$GATE"
printf 'onlyname\n' > "$WORK/manifest-noreason"
t "豁免条目缺理由拒绝" 1 "缺理由" env CHECK_ROOT="$G" PANIC_HYGIENE_EXEMPT="$WORK/manifest-noreason" bash "$GATE"

echo "== 受保护 crate 禁入豁免 =="
printf 'p2p-protocol 理由\n' > "$WORK/manifest-protected"
t "受保护 crate 入清单即红" 1 "禁止加入豁免清单" env CHECK_ROOT="$G" PANIC_HYGIENE_EXEMPT="$WORK/manifest-protected" bash "$GATE"

echo "== 防假绿 =="
t "crates 目录缺失拒绝" 1 "找不到目录" env CHECK_ROOT="$WORK/nocrates" bash "$GATE"
t "真实仓库绿" 0 "panic-hygiene: PASS" env PANIC_HYGIENE_EXEMPT="$REAL_EXEMPT" bash "$GATE"

echo "== 结果：$pass 通过 / $fail 失败 =="
if [ "$fail" -ne 0 ]; then
  echo "panic-hygiene 自测：FAIL" >&2
  exit 1
fi
echo "panic-hygiene 自测：PASS"
