import { afterEach, describe, expect, it, vi } from "vitest";

// 诊断面固定走真实 Tauri IPC（2026-09-03 裁决）：即使 VITE_MOCK_IPC=1，
// diag 也不得回落 mock-diagnostics 读 localStorage——mock 仅测试内 vi.mock 使用。
const invokeMock = vi.hoisted(() =>
  vi.fn((cmd: string) => {
    if (cmd === "frontend_log_path") return Promise.resolve("/tmp/frontend.log");
    if (cmd === "frontend_log_tail") return Promise.resolve(["real-tail"]);
    return Promise.resolve(null);
  }),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const ADDR = "192.168.1.5/u3400";

describe("ipc 诊断面路由", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    invokeMock.mockClear();
    localStorage.clear();
  });

  it("VITE_MOCK_IPC=1 时 diag 仍调用真实 frontend_log IPC", async () => {
    // 埋一个 mock 诊断会返回的内容：若 diag 误回落 localStorage，会读到它。
    localStorage.setItem("p2p-console.frontend-log", '{"kind":"mock-stale"}');
    vi.stubEnv("VITE_MOCK_IPC", "1");
    const { diag } = await import("./ipc");

    await expect(diag.logPath()).resolves.toBe("/tmp/frontend.log");
    await expect(diag.logTail(7)).resolves.toEqual(["real-tail"]);
    expect(invokeMock).toHaveBeenCalledWith("frontend_log_path");
    expect(invokeMock).toHaveBeenCalledWith("frontend_log_tail", { maxLines: 7 });
  });

  it("节点控制面在 VITE_MOCK_IPC=1 仍走 mock（与诊断面分离）", async () => {
    vi.stubEnv("VITE_MOCK_IPC", "1");
    const { ipc } = await import("./ipc");

    const status = await ipc.nodeStatus();
    expect(status).toMatchObject({ running: false });
    expect(invokeMock.mock.calls.map(([cmd]) => cmd)).not.toContain("node_status");
  });
});

describe("ipc chat 命令映射（契约 v7 §12，真实桥接）", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    invokeMock.mockClear();
  });

  it("chat_* 封装逐字映射命令名与 camelCase 参数，可选参缺省传 null", async () => {
    // 本文件前两个用例已按 VITE_MOCK_IPC=1 求值过模块；重置后按 0 重新求值，
    // 才能断言真实 tauriBackend 的 chat 命令映射。
    vi.resetModules();
    vi.stubEnv("VITE_MOCK_IPC", "0");
    const { ipc } = await import("./ipc");

    await ipc.chatFriendsList();
    expect(invokeMock).toHaveBeenCalledWith("chat_friends_list");

    await ipc.chatFriendAdd("p1", "nick", [ADDR]);
    expect(invokeMock).toHaveBeenCalledWith("chat_friend_add", {
      peerId: "p1",
      nickname: "nick",
      addrs: [ADDR],
    });

    await ipc.chatFriendRemove("p1");
    expect(invokeMock).toHaveBeenCalledWith("chat_friend_remove", {
      peerId: "p1",
    });

    await ipc.chatHistory("p1", "cursor-id", 25);
    expect(invokeMock).toHaveBeenCalledWith("chat_history", {
      peer: "p1",
      beforeId: "cursor-id",
      limit: 25,
    });
    await ipc.chatHistory("p1");
    expect(invokeMock).toHaveBeenCalledWith("chat_history", {
      peer: "p1",
      beforeId: null,
      limit: null,
    });

    await ipc.chatSend("p1", "text", "hi");
    expect(invokeMock).toHaveBeenCalledWith("chat_send", {
      peer: "p1",
      kind: "text",
      text: "hi",
      media: null,
      replyTo: null,
    });

    // IM-T46B：replyTo 可选透传（camelCase，对齐 src-tauri reply_to 参数）
    await ipc.chatSend("p1", "text", "hi", undefined, "reply-target-1");
    expect(invokeMock).toHaveBeenCalledWith("chat_send", {
      peer: "p1",
      kind: "text",
      text: "hi",
      media: null,
      replyTo: "reply-target-1",
    });

    await ipc.chatMediaFile("p1", "m1");
    expect(invokeMock).toHaveBeenCalledWith("chat_media_file", {
      peer: "p1",
      messageId: "m1",
    });
  });
});
