// 在线启发色点纯函数（契约 §17.3-1）：以卡签发时刻+TTL 折算剩余寿命，
// 真实心跳面接入前只供色点渲染，禁带「在线/离线」文案。now 由调用方注入
// （组件 render 期禁调 Date.now，react-hooks/purity）。
import type { DiscoveredAgent } from "./types";

export type OnlineLevel = "green" | "yellow" | "gray";

export function onlineLevel(nowSecs: number, row: DiscoveredAgent): OnlineLevel {
  const { card, issuedAtSecs } = row;
  if (issuedAtSecs <= 0 || card.ttlSecs <= 0) return "gray";
  const remaining = issuedAtSecs + card.ttlSecs - nowSecs;
  if (remaining <= 0) return "gray";
  return remaining > card.ttlSecs / 2 ? "green" : "yellow";
}
