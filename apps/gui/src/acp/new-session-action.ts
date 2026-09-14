// 新建会话按钮共用动作（两入口同款）：store 失败信号（resolve false）折算为
// reject，供 AsyncButton 呈 fail 态；错误 toast 由 store 层 notifyActionFailure
// 统一弹（单弹），这里禁止重复弹。
import { useAcpStore } from "./acp-store";

export async function newSessionAction(): Promise<void> {
  const created = await useAcpStore.getState().newSession();
  if (!created) throw new Error("sessionNewFailed");
}
