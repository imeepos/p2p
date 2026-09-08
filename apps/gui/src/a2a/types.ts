// A2A wire 形状（gui-contract §17；真值 = crates/a2a card.rs/agents.rs serde 输出）。
// 本文件只做类型声明，禁止在此改字段名——以 Rust 侧 serde 为唯一真值源。

export type AgentVisibility = "public" | "private" | "local";

/** 技能项（AgentSkill，camelCase wire）。 */
export interface AgentSkillJson {
  id: string;
  name: string;
  description?: string;
  tags?: string[];
}

/** 宿主 agent 定义（AgentDef，admin 面 /a2a/agents 的行形状）。 */
export interface AgentDefJson {
  agentId: string;
  name: string;
  description: string;
  skills: AgentSkillJson[];
  visibility: AgentVisibility;
  enabled: boolean;
  createdAt: number;
}

/** 卡片声明本体（AgentCard payload，card 相 cards/push 帧内）。 */
export interface AgentCardJson {
  agentId: string;
  name: string;
  description: string;
  /** a2a://<hostPeer>/<agentId> */
  url: string;
  hostPeer: string;
  visibility: AgentVisibility;
  capabilities?: { streaming?: boolean };
  skills?: AgentSkillJson[];
  ttlSecs: number;
  version: number;
}

/** SignedCard 信封：GUI 不验签（验签在宿主订阅侧），只消费 payload。 */
export interface SignedCardJson {
  payload: AgentCardJson;
  [key: string]: unknown;
}

/** 发现簿行：卡片 + 信封签发时刻（在线点启发用；无则 0 = 未知）。 */
export interface DiscoveredAgent {
  card: AgentCardJson;
  issuedAtSecs: number;
}

/** 创建入参（admin POST body 的 GUI 侧形状）。 */
export interface AgentCreateInput {
  name: string;
  description: string;
  skills: string[];
  visibility: Exclude<AgentVisibility, "local">;
}
