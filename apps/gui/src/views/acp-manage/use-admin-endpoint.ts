// 本地 ACP 管理页共用接缝：admin 端点解析（登记优先 + 本机描述文件兜底）。
// 与 share-manage-card 同源逻辑收敛为一处；返回原始值（对象每渲染都是新引用，
// 依赖 endpoint 对象的 effect 会自旋——share-manage-card 同款教训）。
import { useAcpStore } from "@/acp/acp-store";
import { adminEndpointCandidates } from "@/acp/admin-endpoints";
import { useLocalAdminCandidate } from "@/acp/use-local-admin";

export function useAdminEndpoint(label: string): {
  endpointUrl: string | null;
  endpointToken: string;
  done: boolean;
} {
  const saved = useAcpStore((s) => s.saved);
  const draft = useAcpStore((s) => s.draft);
  const registered = adminEndpointCandidates(saved, draft)[0] ?? null;
  const { candidate, done } = useLocalAdminCandidate(registered === null, label);
  const endpoint = registered ?? candidate;
  return {
    endpointUrl: endpoint?.url ?? null,
    endpointToken: endpoint?.token ?? "",
    done,
  };
}