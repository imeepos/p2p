#!/usr/bin/env bash
# src-tauri 独立门禁：该 crate 声明空 [workspace] 脱离根 workspace，根
# fmt/clippy/test 全不覆盖（2026-09-04 E0423 编译红潜伏一天实锤）。
# macOS 直接跑 cargo test（系统 WKWebView 无需额外系统库）；Linux（CI
# ubuntu）缺 webkit2gtk-4.1 系统库时 tauri crate 无法编译，显式 SKIP 不
# 假绿；GUI_TAURI_SKIP=1 提供显式逃生口（SKIP 同样可观测）。
set -eu
set -o pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../../apps/gui/src-tauri"

if [ "${GUI_TAURI_SKIP:-0}" = "1" ]; then
    echo "gui-tauri-check: SKIP (GUI_TAURI_SKIP=1 显式跳过)"
    exit 0
fi

if [ "$(uname -s)" != "Darwin" ] && ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
    echo "gui-tauri-check: SKIP (非 macOS 且缺 webkit2gtk-4.1 系统库，tauri crate 无法编译)"
    exit 0
fi

export PATH="$HOME/.cargo/bin:$PATH"
cargo test
echo "gui-tauri-check: PASS"
