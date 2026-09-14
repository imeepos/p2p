// session/new 契约：cwd 必填且为真实路径（owner 本机不经 agent 改写直透子进程，
// 设计「cwd 改写」安全点行）；mcpServers 空数组由 acp-connection 兜底。cwd 取用户
// 主目录（Tauri path API）；纯浏览器（mock dev/测试）回退占位串，降级显式留痕。
import { homeDir } from "@tauri-apps/api/path";

import { isTauriRuntime } from "@/lib/tauri-env";

const MOCK_CWD = "/tmp/p2p-gui-mock-cwd";
let cached: string | null = null;

export async function resolveSessionCwd(): Promise<string> {
  if (cached) return cached;
  if (isTauriRuntime()) {
    try {
      cached = await homeDir();
      if (cached) return cached;
      console.warn("[acp] 主目录解析为空：session cwd 回退占位路径");
    } catch (error) {
      console.warn("[acp] 主目录解析失败：session cwd 回退占位路径", error);
    }
  } else {
    console.info("[acp] 非 Tauri 环境：session cwd 使用占位路径（mock 语义）");
  }
  cached = MOCK_CWD;
  return cached;
}

/** 测试隔离：复位 cwd 缓存 */
export function resetSessionCwdForTest(): void {
  cached = null;
}
