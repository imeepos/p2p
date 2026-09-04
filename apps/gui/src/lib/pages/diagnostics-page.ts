// diagnostics 页 descriptor：与诊断页刷新/清空按钮同源（diag IPC +
// error-report 缓冲）。clearAll 清空错误缓冲与日志文件且不可恢复，为危险动作，
// registry 强制 args.confirm===true。
import { clearErrorBufferAndQueue, getRecentErrors } from "@/lib/error-report";
import { diag } from "@/lib/ipc";
import type { PageDescriptor, PageEntry } from "../page-registry";

const DEFAULT_TAIL_LINES = 50;
const STATE_ERROR_ROWS = 10;

const descriptor: PageDescriptor = {
  name: "diagnostics",
  description: "诊断页：前端错误缓冲与日志尾部观测",
  actions: [
    {
      name: "refresh",
      description: "刷新日志路径与尾部内容（与刷新按钮同源）",
      args: [
        { name: "tailLines", type: "number", required: false, description: "尾部行数，默认 50" },
      ],
    },
    {
      name: "clearAll",
      description: "清空错误缓冲与日志文件（与清空按钮同源，内容不可恢复）",
      confirm: true,
      args: [
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
  ],
};

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  switch (action) {
    case "refresh": {
      const tailLines =
        typeof args.tailLines === "number" ? args.tailLines : DEFAULT_TAIL_LINES;
      const [logPath, tail] = await Promise.all([
        diag.logPath(),
        diag.logTail(tailLines),
      ]);
      return { logPath, tail };
    }
    case "clearAll":
      clearErrorBufferAndQueue();
      await diag.logClear();
      return { cleared: true };
    default:
      throw new Error(`diagnostics 页未知动作: ${action}`);
  }
}

function state(): unknown {
  return {
    recentErrors: getRecentErrors()
      .slice(-STATE_ERROR_ROWS)
      .map((entry) => ({ ts: entry.ts, kind: entry.kind, message: entry.message })),
  };
}

export const diagnosticsPage: PageEntry = { descriptor, execute, state };
