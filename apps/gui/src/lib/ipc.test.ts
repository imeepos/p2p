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

    await ipc.chatFriendInvite("p1", "nick", [ADDR]);
    expect(invokeMock).toHaveBeenCalledWith("chat_friend_invite", {
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

describe("ipc llm-share 命令映射（契约 v11 §16，真实桥接）", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    invokeMock.mockClear();
  });

  it("llm_share_* 封装逐字映射命令名与 camelCase 参数，可选参缺省传 null", async () => {
    vi.resetModules();
    vi.stubEnv("VITE_MOCK_IPC", "0");
    const { ipc } = await import("./ipc");

    const offer = { models: ["gpt-4o"], spare: { "gpt-4o": 1 }, periodEnds: "2026-09-30" };
    await ipc.llmShareOfferPublish(offer);
    expect(invokeMock).toHaveBeenCalledWith("llm_share_offer_publish", { offer });

    await ipc.llmShareOfferShow();
    expect(invokeMock).toHaveBeenCalledWith("llm_share_offer_show");

    await ipc.llmShareAllowList();
    expect(invokeMock).toHaveBeenCalledWith("llm_share_allow_list");

    await ipc.llmShareAllow("p1", ["gpt-4o"], "note");
    expect(invokeMock).toHaveBeenCalledWith("llm_share_allow", {
      peerId: "p1",
      models: ["gpt-4o"],
      note: "note",
    });
    await ipc.llmShareAllow("p1");
    expect(invokeMock).toHaveBeenCalledWith("llm_share_allow", {
      peerId: "p1",
      models: null,
      note: null,
    });

    await ipc.llmShareDeny("p1");
    expect(invokeMock).toHaveBeenCalledWith("llm_share_deny", { peerId: "p1" });

    const req = {
      model: "gpt-4o",
      messages: [{ role: "user" as const, content: "hi" }],
      maxTokens: 1024,
      targetPeer: "p1",
    };
    await ipc.llmShareBorrow(req);
    expect(invokeMock).toHaveBeenCalledWith("llm_share_borrow", { req });

    await ipc.llmShareLedgerList({ lender: "p1" });
    expect(invokeMock).toHaveBeenCalledWith("llm_share_ledger_list", {
      filter: { lender: "p1" },
    });
    await ipc.llmShareLedgerList();
    expect(invokeMock).toHaveBeenCalledWith("llm_share_ledger_list", { filter: null });

    await ipc.llmShareLedgerBalance();
    expect(invokeMock).toHaveBeenCalledWith("llm_share_ledger_balance");

    await ipc.llmShareReceiptVerify("req-1", "pubkey");
    expect(invokeMock).toHaveBeenCalledWith("llm_share_receipt_verify", {
      reqId: "req-1",
      lenderPubkey: "pubkey",
    });
    await ipc.llmShareReceiptVerify("req-1");
    expect(invokeMock).toHaveBeenCalledWith("llm_share_receipt_verify", {
      reqId: "req-1",
      lenderPubkey: null,
    });
  });
});

describe("ipc authz 命令映射（契约 §18，真实桥接）", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    invokeMock.mockClear();
  });

  it("authz_* 封装逐字映射命令名与 camelCase 参数，可选参缺省传 null", async () => {
    vi.resetModules();
    vi.stubEnv("VITE_MOCK_IPC", "0");
    const { ipc } = await import("./ipc");

    await ipc.authzRoleList();
    expect(invokeMock).toHaveBeenCalledWith("authz_role_list");

    await ipc.authzBindingsList();
    expect(invokeMock).toHaveBeenCalledWith("authz_bindings_list");

    await ipc.authzBind("p1", "ally", 1700000000, "note");
    expect(invokeMock).toHaveBeenCalledWith("authz_bind", {
      peerId: "p1",
      roleId: "ally",
      expiresAt: 1700000000,
      note: "note",
    });
    await ipc.authzBind("p1", "ally");
    expect(invokeMock).toHaveBeenCalledWith("authz_bind", {
      peerId: "p1",
      roleId: "ally",
      expiresAt: null,
      note: null,
    });

    await ipc.authzUnbind("p1");
    expect(invokeMock).toHaveBeenCalledWith("authz_unbind", { peerId: "p1" });

    await ipc.authzCheck("p1", "llm.borrow");
    expect(invokeMock).toHaveBeenCalledWith("authz_check", {
      peerId: "p1",
      permission: "llm.borrow",
    });

    await ipc.authzDefaultRoleGet();
    expect(invokeMock).toHaveBeenCalledWith("authz_default_role_get");

    await ipc.authzDefaultRoleSave("guest");
    expect(invokeMock).toHaveBeenCalledWith("authz_default_role_save", {
      roleId: "guest",
    });
  });
});

