// llm-share 页 descriptor：闲置额度共享控制台只读语义登记面。
// 页面数据由组件级异步获取（views/llm-share backend），无同步快照；
// 严禁写路径：actions 留空，执行器恒拒绝。
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "llm-share",
  description:
    "闲置 LLM 额度共享页：概览统计/借用/账本/能力发布/白名单/上游配置六 tab（只读观测）",
  actions: [],
};

async function execute(
  action: string,
  _args: Record<string, unknown>,
): Promise<unknown> {
  throw new Error(`llm-share 页为只读页，无动作可执行: ${action}`);
}

export const llmSharePage: PageEntry = { descriptor, execute };
