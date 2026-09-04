#!/usr/bin/env bash
# CLI 对等守卫（CL4 核心交付，防 GUI/CLI 命令面漂移）：
#   1. 机械提取 apps/gui/src-tauri/src/lib.rs generate_handler![...] 命令全集
#      （兼容 commands::xxx 与 <模块>::xxx 两种形态，取路径末段）；
#   2. 对照 scripts/check/cli-parity.tsv 映射表（机器可读 TSV）；
#   3. mapped 行用 p2pctl --help 实测存在性验证（递归解析 Commands: 段到叶子，
#      覆盖 chat friends list 三层路径），禁止只对表不验实际命令；
#   4. 缺映射 / 映射命令不存在 / 豁免无理由 / 表内陈旧行 → 输出清单并以非 0 退出。
# T22 环境加固：p2pctl 缺失或陈旧（apps/cli/src + crates/p2p-cli/src 任一 .rs 更新）
# 自动重建并输出观测日志；构建显式固定 CARGO_TARGET_DIR，消解外部注入导致的
# 产物落点漂移（CTL 判定路径失真 → 假红）。bash scripts/check/cli-parity.sh
# --self-test 可离线自测两道防御。
# 成功时末行输出 CLI-PARITY-OK。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TSV="${ROOT}/scripts/check/cli-parity.tsv"
GUI_LIB="${ROOT}/apps/gui/src-tauri/src/lib.rs"
CTL="${ROOT}/apps/cli/target/debug/p2pctl"

fail() { echo "cli-parity: FAIL $1" >&2; exit 1; }

# --- 0. T22 环境加固原语 + 离线自测 ---
p2pctl_stale() {  # <ctl> <src-dir>...：二进制缺失或任一源码目录存在更新 .rs → 需重建(0)
    local ctl="$1"; shift
    local dir
    [ -x "${ctl}" ] || return 0
    for dir in "$@"; do
        if [ -n "$(find "${dir}" -name '*.rs' -newer "${ctl}" -print -quit 2>/dev/null)" ]; then
            return 0
        fi
    done
    return 1
}
p2pctl_rebuild() {  # <tag>：固定 CARGO_TARGET_DIR 重建，产物落点与 CTL 判定路径严格一致
    echo "${1}: rebuilt stale p2pctl（缺失或源码更新，固定 target 目录重建 apps/cli）…" >&2
    (cd "${ROOT}" && CARGO_TARGET_DIR="${ROOT}/apps/cli/target" \
        cargo build --manifest-path apps/cli/Cargo.toml -q)
}
self_test() {  # 伪造临时树断言新鲜度判定与 target 隔离，不触发真实 cargo 构建
    # self_test_tmp 须为全局：EXIT trap 触发时函数已返回，local 变量已出作用域
    local src src2 ctl fakebin out
    self_test_tmp="$(mktemp -d)"
    trap 'rm -rf "${self_test_tmp}"' EXIT
    src="${self_test_tmp}/src"; src2="${self_test_tmp}/src2"; ctl="${self_test_tmp}/ctl"
    mkdir "${src}" "${src2}" "${self_test_tmp}/bin"
    fakebin="${self_test_tmp}/bin"
    p2pctl_stale "${ctl}" "${src}" "${src2}" \
        || { echo "self-test: 二进制缺失未判需重建" >&2; return 1; }
    touch "${src}/a.rs" "${src2}/b.rs"; : > "${ctl}"; chmod +x "${ctl}"
    if p2pctl_stale "${ctl}" "${src}" "${src2}"; then
        echo "self-test: 新鲜二进制被误判陈旧" >&2; return 1
    fi
    touch "${src2}/b.rs"
    p2pctl_stale "${ctl}" "${src}" "${src2}" \
        || { echo "self-test: 源码更新未判陈旧（自愈未触发）" >&2; return 1; }
    touch "${ctl}"; touch "${src}/notes.txt"
    if p2pctl_stale "${ctl}" "${src}" "${src2}"; then
        echo "self-test: 非 .rs 变更误触发重建" >&2; return 1
    fi
    printf '#!/usr/bin/env bash\nprintf "%%s" "${CARGO_TARGET_DIR-UNSET}"\n' > "${fakebin}/cargo"
    chmod +x "${fakebin}/cargo"
    out="$(CARGO_TARGET_DIR=/tmp/t22-injected-env PATH="${fakebin}:${PATH}" \
        p2pctl_rebuild self-test)" \
        || { echo "self-test: p2pctl_rebuild 失败" >&2; return 1; }
    [ "${out}" = "${ROOT}/apps/cli/target" ] \
        || { echo "self-test: CARGO_TARGET_DIR 隔离失效，cargo 实收 '${out}'" >&2; return 1; }
    echo "SELF-TEST-OK"
}
if [ "${1:-}" = "--self-test" ]; then
    self_test
    exit 0
fi

[ -f "${GUI_LIB}" ] || fail "找不到 ${GUI_LIB}"
[ -f "${TSV}" ] || fail "找不到映射表 ${TSV}"

# --- 1. GUI 命令全集提取 ---
gui_cmds="$(awk '/generate_handler!\[/{flag=1} flag{print} flag && /\]/{exit}' "${GUI_LIB}" \
  | grep -oE '[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*' \
  | awk -F'::' '{print $NF}' \
  | grep -v '^generate_handler$' \
  | sort -u)"
[ -n "${gui_cmds}" ] || fail "未能从 generate_handler! 提取到任何命令"

gui_has() { printf '%s\n' "${gui_cmds}" | grep -Fxq "$1"; }

# --- 2. p2pctl 就位（缺失或源码比二进制新即重建；构建固定 CARGO_TARGET_DIR） ---
export PATH="${HOME}/.cargo/bin:${PATH}"
if p2pctl_stale "${CTL}" "${ROOT}/apps/cli/src" "${ROOT}/crates/p2p-cli/src"; then
    p2pctl_rebuild cli-parity || fail "p2pctl 构建失败"
fi

# --- 3. 实测收集 CLI 命令面（递归 --help 的 Commands: 段到叶子） ---
cli_cmds_file="$(mktemp)"
trap 'rm -f "${cli_cmds_file}"' EXIT
collect() {
    local path="$*" out subs sub
    out="$("${CTL}" $path --help 2>&1)" || fail "执行 '$path --help' 失败"
    subs="$(printf '%s\n' "${out}" \
        | awk '/^Commands:/{f=1;next} f&&/^[[:space:]]*$/{exit} f{print}' \
        | awk 'NF{print $1}' | grep -v '^help$' || true)"
    if [ -z "${subs}" ]; then
        printf '%s\n' "${path}" >> "${cli_cmds_file}"
        return
    fi
    for sub in ${subs}; do
        collect $path $sub
    done
}
collect ""

cli_has() { grep -Fxq "$1" "${cli_cmds_file}"; }

# --- 4. 对照映射表 ---
missing_mapping=""
stale_rows=""
bad_cmd=""
bad_exempt=""
mapped_count=0
exempt_count=0
while IFS= read -r tsv_line; do
    case "${tsv_line}" in ''|'#'*) continue ;; esac
    # cut 按列取值：read + IFS 会把连续 TAB 折叠成单分隔符，空字段（如豁免行的
    # invocation）会被吞掉，导致理由列错位——这里必须用 cut 保空字段。
    gui="$(printf '%s\n' "${tsv_line}" | cut -f1)"
    kind="$(printf '%s\n' "${tsv_line}" | cut -f2)"
    invocation="$(printf '%s\n' "${tsv_line}" | cut -f3)"
    reason="$(printf '%s\n' "${tsv_line}" | cut -f4-)"
    if ! gui_has "${gui}"; then
        stale_rows="${stale_rows}  ${gui}（映射表有行，GUI 已无此命令）\n"
        continue
    fi
    case "${kind}" in
        mapped)
            if [ -z "${invocation}" ] || ! cli_has "${invocation}"; then
                bad_cmd="${bad_cmd}  ${gui} → [无 invocation]（p2pctl 实测无此命令）\n"
            else
                mapped_count=$((mapped_count + 1))
            fi
            ;;
        exempt)
            trimmed="$(printf '%s' "${reason}" | tr -d ' ')"
            if [ -z "${trimmed}" ]; then
                bad_exempt="${bad_exempt}  ${gui}（豁免缺理由）\n"
            else
                exempt_count=$((exempt_count + 1))
            fi
            ;;
        *)
            bad_cmd="${bad_cmd}  ${gui} → kind 非法（只允许 mapped|exempt）\n"
            ;;
    esac
done < "${TSV}"

tsv_gui_column="$(grep -v '^#' "${TSV}" | cut -f1)"
while IFS= read -r gui; do
    if ! printf '%s\n' "${tsv_gui_column}" | grep -Fxq "${gui}"; then
        missing_mapping="${missing_mapping}  ${gui}（GUI 有命令，映射表无行）\n"
    fi
done <<< "${gui_cmds}"

# --- 5. 汇总裁决 ---
rc=0
report() {
    local label="$1" list="$2"
    if [ -n "${list}" ]; then
        echo "cli-parity: ${label}" >&2
        printf '%b' "${list}" >&2
        rc=1
    fi
}
report "缺映射清单：" "${missing_mapping}"
report "映射命令不存在清单：" "${bad_cmd}"
report "豁免缺理由清单：" "${bad_exempt}"
report "陈旧映射行清单：" "${stale_rows}"
if [ "${rc}" -ne 0 ]; then
    echo "cli-parity: FAIL" >&2
    exit 1
fi
leaf_count="$(grep -c . "${cli_cmds_file}")"
gui_count="$(printf '%s\n' "${gui_cmds}" | wc -l | tr -d ' ')"
echo "cli-parity: GUI 命令 ${gui_count} 个，映射 ${mapped_count}，豁免 ${exempt_count}；p2pctl 实测叶子命令 ${leaf_count} 个"
echo "CLI-PARITY-OK"
