import type { TunnelServeStatus } from "./ipc-types";

// tunnel 被访侧 mock（gui-contract §19.1）：内存白名单 + 受理开关。
// 与 tunnel.rs 同语义：节点未运行拒绝（gate 未装配）、先校验后动作
// 不得部分生效（target 非法时白名单/enabled 均不变）、stop 幂等且保留白名单。

export interface MockTunnelServeDeps {
  isRunning: () => boolean;
}

const TARGET_RE = /^127\.0\.0\.1:(\d{1,5})$/;

export function createMockTunnelServe(deps: MockTunnelServeDeps) {
  const state = {
    enabled: false,
    allow: [] as string[],
    activeSessions: 0,
  };

  function requireRunning(): void {
    if (!deps.isRunning()) {
      throw new Error("节点未运行，tunnel 不可用");
    }
  }

  function requireValidTarget(target: string): void {
    const matched = TARGET_RE.exec(target);
    const port = matched ? Number(matched[1]) : 0;
    if (!matched || port < 1 || port > 65535) {
      throw new Error(
        `目标须为 127.0.0.1:<port> 字面量（端口 1-65535），当前: ${target}`,
      );
    }
  }

  function snapshot(): TunnelServeStatus {
    return {
      enabled: state.enabled,
      allow: [...state.allow],
      activeSessions: state.activeSessions,
    };
  }

  const backend = {
    async tunnelServeStart(target: string): Promise<TunnelServeStatus> {
      await delay(150);
      requireRunning();
      requireValidTarget(target);
      if (!state.allow.includes(target)) state.allow.push(target);
      state.enabled = true;
      return snapshot();
    },

    async tunnelServeStop(): Promise<TunnelServeStatus> {
      await delay(150);
      requireRunning();
      state.enabled = false;
      return snapshot();
    },
  };

  // dev 注入入口：复位会话态（窗口/测试隔离共用）。
  const controller = {
    reset(): void {
      state.enabled = false;
      state.allow = [];
      state.activeSessions = 0;
    },
  };

  return { backend, controller, status: snapshot };
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}
