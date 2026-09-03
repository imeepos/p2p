// 浏览器/mock 模式诊断后端（契约 §8）：以 localStorage 键模拟 frontend.log 的读路径，
// 写路径复用 error-report 的降级持久化，两端同键同格式。
import { readLocalLogLines } from "./error-report";

import type { DiagBackend } from "./ipc-types";

export const MOCK_LOG_LABEL = "localStorage:p2p-console.frontend-log";

export const mockDiagBackend: DiagBackend = {
  async logPath() {
    return MOCK_LOG_LABEL;
  },

  async logTail(maxLines) {
    const lines = readLocalLogLines();
    return lines.slice(Math.max(0, lines.length - Math.max(1, maxLines)));
  },
};