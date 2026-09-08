// 契约 gui-contract §16.6 v13（llm-share-link 波）：双协议 provider + 分享链接 8 命令面 DTO。
// 独立文件：ipc-types.ts（IPC 接缝）与 views/llm-share/types.ts（视图接缝）共用同一组
// 类型，避免双份定义漂移；字段与契约表逐字对齐（camelCase）。

export type LlmProviderProtocol = "openai" | "claude";

// providerList 返回：apiKey 只出掩码（明文仅存在于 0600 密钥文件，永不过 IPC 回传）。
export interface LlmProviderView {
  id: string;
  name: string;
  baseUrl: string;
  protocol: LlmProviderProtocol;
  models: string[];
  createdAt: number;
  apiKeyMasked: string;
}

// providerSave 入参：apiKey 明文仅入参一次（落 0600 密钥文件）；更新时留空 = 保留原密钥。
export interface LlmProviderSaveReq {
  id?: string;
  name: string;
  baseUrl: string;
  protocol: LlmProviderProtocol;
  apiKey?: string;
  models: string[];
}

// shareCreate 入参：models 缺省 = provider 全模型且须 ⊆ offer.models；maxActivations 固定 1。
export interface LlmShareCreateReq {
  providerId: string;
  models?: string[];
  expiresAt?: number;
  maxActivations?: number;
  note?: string;
}

// token 原文只在这条响应出现一次；link 即兑换凭证（dsh-llm-share://）。
export interface LlmShareCreateResult {
  link: string;
  shareId: string;
  expiresAt: number;
  models: string[];
}

// shareList 返回：永不含 token/明文 key；status 单徽章推导优先级 撤销>过期>用尽>有效。
export interface LlmShareEntry {
  shareId: string;
  providerId: string;
  models: string[];
  maxActivations: number;
  activations: number;
  expiresAtUnix: number;
  revoked: boolean;
  note: string;
  createdAt: number;
  boundPeer: string | null;
  status: "active" | "expired" | "revoked" | "exhausted";
}

// 兑换业务拒绝码：原样透出非 Err（对齐 §16.2-1 四值拒绝码先例）。
export type LlmShareRejectCode =
  | "share-revoked"
  | "expired"
  | "exhausted"
  | "bound-other"
  | "invalid";

// shareRedeem 响应内嵌 offer 快照：向导预填 targetPeer=offer.peer、模型候选=offer.models。
export interface LlmShareRedeemResult {
  offer: {
    peer: string;
    models: string[];
    spare: Record<string, number>;
    periodEnds: string;
  };
  shareId: string;
  owner: string;
}

// serveStatus：assembled:false 是常态非故障（未发布 offer 或 provider 缺失），
// lastError 供面板显式告警；装配输入是启动快照，运行中变更不热更。
export interface LlmServeStatus {
  assembled: boolean;
  providerId?: string;
  models: string[];
  lastError?: string;
}
