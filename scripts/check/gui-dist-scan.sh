#!/usr/bin/env bash
# dist 产物 mock 特征扫描：生产不许出现假数据的第二道网。
# 第一道网是 apps/gui/vite.config.ts 的构建期断言（no-mock-in-build）；
# 本扫描兜住绕过 vite 断言的产物（断言被移除的回归、外带 dist 等）。
# 特征清单经双向实测校准（2026-09-05）：干净构建零命中（不误报），
# VITE_MOCK_IPC=1 泄漏构建必现 mock-*.js chunk（必命中）。
# 用法：gui-dist-scan.sh [dist目录]（默认 <repo>/apps/gui/dist）
set -u
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST="${1:-$ROOT/apps/gui/dist}"

if [ ! -d "$DIST" ]; then
  echo "gui-dist-scan: FAIL 产物目录不存在：$DIST（先跑 pnpm build）" >&2
  exit 1
fi

# 内容特征：mock 模块标识符与动态 import 的 chunk 引用串（压缩后仍存活）
content_hits="$(grep -rEl 'mockBackend|mock-ipc|mock-acp-ws|mock-update|mock-chat|VITE_MOCK_IPC' "$DIST" 2>/dev/null || true)"
# 文件名特征：泄漏的 mock chunk 以 mock-*.js 落盘
name_hits="$(find "$DIST" -type f -name '*mock*' 2>/dev/null || true)"

if [ -n "$content_hits" ] || [ -n "$name_hits" ]; then
  echo "gui-dist-scan: FAIL 产物出现 mock 特征——VITE_MOCK_IPC=1 疑似泄漏进生产构建" >&2
  printf '%s\n' "$content_hits" "$name_hits" | sed '/^$/d; s/^/  命中: /' >&2
  exit 1
fi
echo "gui-dist-scan: PASS（$DIST 无 mock 特征）"
