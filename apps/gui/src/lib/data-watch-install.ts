// W1 数据感知装配（组合根 main.tsx 调用）：启动单监听器并注册 store 级
// reloader。config 域由 use-gui-config 按挂载的视图注册（定向刷新语义）。
import { useChatStore } from "@/stores/chat-store";
import { useNodeStore } from "@/stores/node-store";
import { useProfileStore } from "@/stores/profile-store";

import { registerReloader, startDataWatch } from "./data-watch";

export function installDataWatch(): void {
  void startDataWatch();
  // profile_get 与节点无关，node 未启动也可重载；load 失败已置 loadError 并
  // console（profile-store 内），此处不重复上抛。
  registerReloader("profile", () => {
    void useProfileStore.getState().load().catch(() => undefined);
  });
  // chat_friends_list 依赖运行中节点：节点未启动时 chat 视图本就不可用，
  // 跳过重载（下一轮节点启动后的 loadFriends 会拉到最新好友簿）。
  registerReloader("chat", () => {
    if (!useNodeStore.getState().status?.running) return;
    void useChatStore.getState().loadFriends();
  });
}
