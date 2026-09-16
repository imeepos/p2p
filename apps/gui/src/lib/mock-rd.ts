import type { RdFrameListener, RdHostStatus, RdViewerStatus } from "./ipc-types";

import { startFramePump, type MockFramePump } from "./mock-rd-frame-pump";

// rd 远程桌面 mock（gui-contract §21/§21.4）：内存会话态（host 服务开关/审批
// 队列/viewer 会话）+ 帧泵 + 输入录制。与 rd.rs 同语义：节点未运行拒绝、
// host 默认关、审批开时 viewer 连接被拒（awaiting_approval）直至 approve。

export interface MockRdDeps {
  isRunning: () => boolean;
}

export function createMockRd(deps: MockRdDeps) {
  const state = {
    running: false,
    requireApproval: true,
    sessionCount: 0,
    pendingApprovals: [] as string[],
    fps: 15,
    viewerConnected: false,
    viewerSessionId: null as string | null,
    framePump: null as MockFramePump | null,
    lastInput: null as string | null,
  };

  function requireRunning(): void {
    if (!deps.isRunning()) {
      throw new Error("节点未运行，rd 不可用");
    }
  }

  function hostSnapshot(): RdHostStatus {
    return {
      running: state.running,
      requireApproval: state.requireApproval,
      sessionCount: state.sessionCount,
      pendingApprovals: [...state.pendingApprovals],
      fps: state.fps,
    };
  }

  const backend = {
    async rdHostStart(requireApproval: boolean): Promise<RdHostStatus> {
      await delay(120);
      requireRunning();
      state.requireApproval = requireApproval;
      state.running = true;
      return hostSnapshot();
    },

    async rdHostStop(): Promise<RdHostStatus> {
      await delay(120);
      requireRunning();
      state.running = false;
      state.sessionCount = 0;
      state.pendingApprovals = [];
      return hostSnapshot();
    },

    async rdHostStatus(): Promise<RdHostStatus> {
      requireRunning();
      return hostSnapshot();
    },

    async rdApprove(peer: string): Promise<boolean> {
      await delay(80);
      requireRunning();
      const idx = state.pendingApprovals.indexOf(peer);
      if (idx >= 0) {
        state.pendingApprovals.splice(idx, 1);
        return true;
      }
      return false;
    },

    async rdDeny(peer: string): Promise<boolean> {
      await delay(80);
      requireRunning();
      const idx = state.pendingApprovals.indexOf(peer);
      if (idx >= 0) {
        state.pendingApprovals.splice(idx, 1);
        return true;
      }
      return false;
    },

    async rdQualitySet(fps: number, scale: number, codec: number): Promise<RdHostStatus> {
      await delay(80);
      requireRunning();
      if (fps >= 1 && fps <= 60) state.fps = fps;
      void scale;
      void codec;
      return hostSnapshot();
    },

    async rdViewerConnect(
      peer: string,
      sessionId?: string,
      onFrame?: RdFrameListener,
    ): Promise<RdViewerStatus> {
      await delay(150);
      requireRunning();
      if (state.requireApproval && state.pendingApprovals.length > 0) {
        // mock 简化：待审批存在即拒（真实语义=peer 级，E2E 在 Rust 层覆盖）
        throw new Error("握手拒绝: awaiting_approval");
      }
      state.viewerConnected = true;
      state.viewerSessionId = sessionId ?? randomSession();
      state.sessionCount = 1;
      void peer;
      if (onFrame) {
        state.framePump = startFramePump(onFrame);
      }
      return { connected: true, sessionId: state.viewerSessionId };
    },

    async rdViewerClose(): Promise<RdViewerStatus> {
      await delay(80);
      stopPump();
      state.viewerConnected = false;
      state.viewerSessionId = null;
      state.sessionCount = 0;
      return { connected: false, sessionId: null };
    },

    async rdViewerStatus(): Promise<RdViewerStatus> {
      return {
        connected: state.viewerConnected,
        sessionId: state.viewerSessionId,
      };
    },

    async rdInputMouse(
      x: number,
      y: number,
      buttons: number,
      wheelDx: number,
      wheelDy: number,
    ): Promise<boolean> {
      if (!state.viewerConnected) return false;
      state.lastInput = `mouse(${x},${y},b${buttons},w${wheelDx},${wheelDy})`;
      return true;
    },

    async rdInputKey(code: number, down: boolean, modifiers: number): Promise<boolean> {
      if (!state.viewerConnected) return false;
      state.lastInput = `key(${code},${down ? "down" : "up"},m${modifiers})`;
      return true;
    },

    async rdInputKeyReset(): Promise<boolean> {
      if (!state.viewerConnected) return false;
      state.lastInput = "key-reset";
      return true;
    },
  };

  function stopPump(): void {
    state.framePump?.stop();
    state.framePump = null;
  }

  // dev 注入入口：复位会话态；测试可经 controller 预置审批队列。
  const controller = {
    reset(): void {
      stopPump();
      state.running = false;
      state.requireApproval = true;
      state.sessionCount = 0;
      state.pendingApprovals = [];
      state.fps = 15;
      state.viewerConnected = false;
      state.viewerSessionId = null;
      state.lastInput = null;
    },
    queueApproval(peer: string): void {
      if (!state.pendingApprovals.includes(peer)) {
        state.pendingApprovals.push(peer);
      }
    },
    setRunning(v: boolean): void {
      state.running = v;
    },
    lastInput(): string | null {
      return state.lastInput;
    },
  };

  return { backend, controller, status: hostSnapshot };
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function randomSession(): string {
  let out = "";
  for (let i = 0; i < 16; i += 1) {
    out += Math.floor(Math.random() * 16).toString(16);
  }
  return out;
}