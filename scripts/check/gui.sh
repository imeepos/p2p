#!/usr/bin/env bash
# GUI 门禁：lint + tsc/vite build + vitest（含整应用启动冒烟）。
# 背景：2026-09-02 白屏事故——make check 全绿但 GUI 运行时崩溃，
# 因 check 此前只覆盖 Rust，GUI 零门禁。
set -eu
set -o pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../../apps/gui"

pnpm lint
pnpm build
pnpm test
echo "gui-check: PASS"
