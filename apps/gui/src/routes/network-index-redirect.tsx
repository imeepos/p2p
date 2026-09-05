import { recallNetworkTab } from "@/views/network/network-tab-memory";

import { QueryRedirect } from "./redirects";

// /network 索引归一（4.1 tab 记忆）：会话内停留过则落最后停留 tab，
// 全新会话落 overview；query 合并统一经 mergeRedirectQuery，禁止手写拼接。
export function NetworkIndexRedirect() {
  return <QueryRedirect to={`/network/${recallNetworkTab()}`} />;
}
