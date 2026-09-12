// 本机 agent PeerId 零配置解析（2026-09-12 用户裁决：开箱即连，不依赖发现面）：
// 数据源 = acp-agent 落盘的自描述文件（~/.dsh/acp/local-agent.json，0600，peer 为
// agent 节点持久身份，跨重启稳定）。发现面（mDNS/rendezvous）是启发式旁路，仅在
// 描述缺失（agent 从未在本机跑过）时作为回落；两者都失败才挂起并显式留痕。
import { ipc } from "@/lib/ipc";

/** 读取本机 agent 自描述并取 PeerId；缺失/无 peer 返回 null（回落发现面），失败留痕。 */
export async function fetchLocalAgentPeer(): Promise<string | null> {
  try {
    const descriptor = await ipc.acpLocalDescriptor();
    return descriptor?.peer ?? null;
  } catch (error) {
    console.warn("[acp] 本机 agent 描述读取失败：peer 解析回落发现面", error);
    return null;
  }
}
