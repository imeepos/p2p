#!/usr/bin/env bash
# GUI 门禁：lint + tsc/vite build + dist mock 扫描 + vitest（含整应用启动冒烟）。
# 背景：2026-09-02 白屏事故——make check 全绿但 GUI 运行时崩溃，
# 因 check 此前只覆盖 Rust，GUI 零门禁。
# dist 扫描：VITE_MOCK_IPC=1 经 shell 泄漏时 vite 构建期断言拦构建（第一道网），
# 本扫描兜产物（第二道网），双网防 mock 进生产（2026-09-05 审计）。
set -eu
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/../../apps/gui"

pnpm lint
pnpm build
bash "$SCRIPT_DIR/gui-dist-scan.sh" dist
pnpm test
echo "gui-check: PASS"
