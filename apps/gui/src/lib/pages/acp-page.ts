// acp 页 descriptor：/acp 控制台的 agent 语义登记面。actions 与
// acp-store 公开方法同源（真实 store 调用，无 DOM 模拟）；store 失败
// 路径只记 lastError 不抛错，execute 以 lastError 增量检测转
// ACTION_FAILED 结构化返回。
import { useAcpStore } from "@/acp/acp-store";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "acp",
  description: "ACP 控制台页：agent 连接生命周期与会话驱动",
  actions: [
    {
      name: "connect",
      description: "按当前草稿端点连接 acp-console WS（与连接按钮同源）",
      args: [],
    },
    {
      name: "disconnect",
      description: "断开当前连接并回到 idle",
      args: [],
    },
    {
      name: "newSession",
      description: "创建新会话并置为活跃",
      args: [],
    },
    {
      name: "refreshSessions",
      description: "刷新会话清单（只读，不产生新会话）",
      args: [],
    },
    {
      name: "sendPrompt",
      description: "向活跃会话发送提示词并等待回合同步",
      args: [
        { name: "text", type: "string", required: true, description: "提示词文本" },
      ],
    },
    {
      name: "closeSession",
      description: "关闭指定会话并清理其 transcript",
      args: [
        { name: "sessionId", type: "string", required: true, description: "目标会话 id" },
      ],
    },
  ],
};

function acpSnapshot(): unknown {
  const s = useAcpStore.getState();
  return { phase: s.phase, sessions: s.sessions, activeSessionId: s.activeSessionId };
}

/** store 失败不抛错只记 lastError：本次执行新增的 lastError 才归因为本动作 */
function failOnNewLastError(before: string | null): void {
  const after = useAcpStore.getState().lastError;
  if (after && after !== before) throw new Error(`acp:${after}`);
}

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const store = useAcpStore.getState();
  const lastErrorBefore = store.lastError;
  switch (action) {
    case "connect":
      await store.connect();
      break;
    case "disconnect":
      store.disconnect();
      break;
    case "newSession":
      await store.newSession();
      break;
    case "refreshSessions":
      await store.refreshSessions();
      break;
    case "sendPrompt":
      await store.sendPrompt(String(args.text));
      break;
    case "closeSession":
      await store.closeSession(String(args.sessionId));
      break;
    default:
      throw new Error(`acp 页未知动作: ${action}`);
  }
  failOnNewLastError(lastErrorBefore);
  return acpSnapshot();
}

export const acpPage: PageEntry = { descriptor, execute, state: acpSnapshot };
