// A2A 卡片事件通道客户端（gui-contract §17.1）：连 console WS（proto=a2a），
// 连上即 list + subscribe；cards/push 按 hostPeer/agentId 键 + version 升序入簿，
// removed 即除名；error 帧原样回调（UI 上浮，禁静默）。通道断开由调用方重拨。
import { decodeFrame, NdjsonAssembler } from "@/acp/ndjson";
import { resolveWsFactory, type WsLike } from "@/acp/ws-factory";

import type { AgentCardJson, DiscoveredAgent, SignedCardJson } from "./types";

export type CardChannelStatus = "connecting" | "online" | "offline";

export interface CardChannelCallbacks {
  onCards: (cards: DiscoveredAgent[]) => void;
  onRemoved: (keys: string[]) => void;
  onError: (message: string) => void;
  onStatus: (status: CardChannelStatus) => void;
}

function payloadOf(envelope: unknown): DiscoveredAgent | null {
  if (!envelope || typeof envelope !== "object") return null;
  const signed = envelope as SignedCardJson;
  const payload = signed.payload;
  if (!payload || typeof payload !== "object") return null;
  const card = payload as AgentCardJson;
  if (!card.agentId || !card.hostPeer) return null;
  const issuedAt = signed.issued_at;
  return {
    card,
    issuedAtSecs: typeof issuedAt === "number" ? issuedAt : 0,
  };
}

/** 卡片键（与宿主 removed 帧同形状 hostPeer/agentId）。 */
export function cardKey(card: AgentCardJson): string {
  return card.hostPeer + "/" + card.agentId;
}

/** version 升序才覆盖（AgentBook 同款钳制，防旧卡覆盖新卡）。 */
export function shouldReplace(current: AgentCardJson | undefined, next: AgentCardJson): boolean {
  return !current || next.version > current.version;
}

/** 收帧分发（裸函数便于单测）：cards/push/error 三面，其余帧忽略。 */
export function handleFrame(
  frame: Record<string, unknown>,
  cb: CardChannelCallbacks,
): void {
  const op = typeof frame.op === "string" ? frame.op : "";
  if (op === "cards" || op === "push") {
    const raw = Array.isArray(frame.cards) ? frame.cards : [];
    const cards = raw.map(payloadOf).filter((row): row is DiscoveredAgent => row !== null);
    if (cards.length > 0) cb.onCards(cards);
    if (op === "push" && Array.isArray(frame.removed)) {
      cb.onRemoved(frame.removed.filter((k): k is string => typeof k === "string"));
    }
    return;
  }
  if (op === "error") {
    const code = typeof frame.code === "string" ? frame.code : "unknown";
    const message = typeof frame.message === "string" ? frame.message : "";
    cb.onError(code + (message ? ": " + message : ""));
  }
}

export class CardChannel {
  private ws: WsLike | null = null;

  constructor(private readonly cb: CardChannelCallbacks) {}

  /** 建连并拉全量 + 订阅；失败经 onError/onStatus 上报，不抛出打断调用方。 */
  connect(wsUrl: string, token: string, hostPeer: string): void {
    this.close();
    this.cb.onStatus("connecting");
    const url =
      wsUrl.replace(/\/+$/, "") +
      "/?token=" + encodeURIComponent(token) +
      "&peer=" + encodeURIComponent(hostPeer) +
      "&proto=a2a";
    let ws: WsLike;
    try {
      ws = resolveWsFactory()(url);
    } catch (error) {
      console.warn("[a2a] 卡片通道建连失败", error);
      this.cb.onStatus("offline");
      return;
    }
    this.ws = ws;
    // 行重组器闭包持有（连接生命周期 = 对象生命周期，无需落字段）
    const assembler = new NdjsonAssembler();
    ws.onopen = () => {
      this.cb.onStatus("online");
      // list 拉全量快照，subscribe 幂等登记变更推送（契约 §17.1 纪律）
      ws.send(JSON.stringify({ op: "list", v: 1, id: 1 }));
      ws.send(JSON.stringify({ op: "subscribe", v: 1, id: 2 }));
    };
    ws.onmessage = (ev) => {
      void decodeFrame(ev.data)
        .then((text) => {
          for (const line of assembler.push(text)) this.dispatch(line);
        })
        .catch((error) => console.warn("[a2a] 帧解码失败", error));
    };
    ws.onerror = (ev) => {
      console.warn("[a2a] 卡片通道错误", ev?.message);
      this.cb.onError(ev?.message ?? "channel error");
    };
    ws.onclose = () => {
      this.cb.onStatus("offline");
    };
  }

  private dispatch(line: string): void {
    let frame: Record<string, unknown>;
    try {
      frame = JSON.parse(line) as Record<string, unknown>;
    } catch {
      console.warn("[a2a] 卡片通道收到非 JSON 行（丢弃并计数）");
      return;
    }
    handleFrame(frame, this.cb);
  }

  close(): void {
    this.ws?.close(1000, "channel closed");
    this.ws = null;
  }
}
