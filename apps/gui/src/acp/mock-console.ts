// mock console 单例盒：会话/发现清单/应答登记等共享状态，跨重连存活供侧栏断言。
// 关断码与发现契约按 2026-09-05 真机对拍实测对齐
//（docs/notes/2026-09-05-acp-real-calibration.md）。
import type { SessionSummary } from "./protocol";
import { DEFAULT_MOCK_CONFIG, type MockConsoleConfig } from "./mock-script";
import type { MockSocket } from "./mock-acp-ws";

/** 发现清单接线口：console discovery 快照 -> store（测试/后续 tauri 转发接线） */
export type DiscoverySink = (peers: Array<{ peer: string; addrs: string[] }>) => void;

export class MockAcpConsole {
  config: MockConsoleConfig = { ...DEFAULT_MOCK_CONFIG };
  sessions = new Map<string, SessionSummary>();
  live: MockSocket[] = [];
  /** 客户端应答帧（request_permission outcome 等），按到达序累积供断言 */
  responses: Array<{ id: number; result?: unknown; error?: unknown }> = [];
  /** 已发出的权限请求帧 id 供测试定位 */
  permissionRequests: Array<{ id: number; sessionId: string; toolKind: string }> = [];
  discoveryPeers: Array<{ peer: string; addrs: string[] }> = [];
  onDiscovery: DiscoverySink | null = null;
  private sessionSeq = 0;
  permissionSeq = 100;

  configure(patch: Partial<MockConsoleConfig>): void {
    this.config = { ...this.config, ...patch };
  }

  reset(): void {
    this.config = { ...DEFAULT_MOCK_CONFIG };
    this.sessions.clear();
    this.live = [];
    this.sessionSeq = 0;
    this.permissionSeq = 100;
    this.responses = [];
    this.permissionRequests = [];
    this.discoveryPeers = [];
    this.onDiscovery = null;
  }

  nextSessionId(): string {
    this.sessionSeq += 1;
    return "s-" + String(this.sessionSeq).padStart(3, "0");
  }

  broadcast(method: string, params: unknown): void {
    for (const socket of this.live) socket.serverPush(method, params);
  }

  /** 对端断流：真机实测 agent 死亡后 console 不发 Close 帧，客户端见 1006 空 reason
   *  （优雅 EOF 路径才有 1000 "peer closed"，用 serverClose(1000, "peer closed") 显式模拟） */
  dropAll(code = 1006, reason = ""): void {
    for (const socket of [...this.live]) socket.serverClose(code, reason);
  }

  /** 桥约定：重连补放通知（dsh/bridge/reattach，无 id） */
  pushReattach(replayed: number): void {
    this.broadcast("dsh/bridge/reattach", { replayed });
  }

  /** 发现清单变更：快照推给已接线的 sink（console stdout/discovery 契约形状） */
  emitDiscovery(): void {
    if (!this.onDiscovery) return;
    this.onDiscovery(this.discoveryPeers.map((p) => ({ ...p, addrs: [...p.addrs] })));
  }

  remove(socket: MockSocket): void {
    this.live = this.live.filter((s) => s !== socket);
  }
}

export const mockAcpConsole = new MockAcpConsole();
