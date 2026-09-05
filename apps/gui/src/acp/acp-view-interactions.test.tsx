// ACP 控制台交互面组件测试（设计 §8 剩余行）：工具时间线、request_permission
// 三分支（批准/拒绝/倒计时归零）、配置下拉（agent 真实目录）、用量条、
// 续连补放横幅、连接目录（发现+手动+scope）。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const {
  newSession,
  permissionId,
  pickOption,
  renderConnected,
  resetFixtures,
  sendPrompt,
} = await import("./acp-view-test-utils");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
});

describe("AcpView tool timeline", () => {
  it("时间线节点随 tool_call_update 迁移状态，入参/结果可见", async () => {
    mockAcpConsole.configure({
      promptScript: [
        { kind: "tool_call", toolCallId: "call-1", title: "Reading config", callKind: "read", input: { path: "a.ts" } },
        { kind: "tool_update", toolCallId: "call-1", status: "in_progress" },
        { kind: "tool_update", toolCallId: "call-1", status: "completed", outputText: "3 files found" },
        { kind: "message", text: "done" },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    const node = await screen.findByTestId("acp-turn-tool-call-1");
    expect(node.textContent).toContain("Reading config");
    expect(node.textContent).toContain("读取");
    await waitFor(() => {
      expect(screen.getByTestId("acp-tool-status-call-1").textContent).toContain("已完成");
    });
    expect(screen.getByTestId("acp-tool-input-call-1").textContent).toContain("a.ts");
    expect(screen.getByTestId("acp-tool-output-call-1").textContent).toContain("3 files found");
  });

  it("失败态与 stopReason 徽章文案 i18n", async () => {
    mockAcpConsole.configure({
      promptScript: [
        { kind: "tool_call", toolCallId: "call-9", title: "Run tests", callKind: "execute" },
        { kind: "tool_update", toolCallId: "call-9", status: "failed", outputText: "exit 1" },
        { kind: "message", text: "no" },
        { kind: "stop", reason: "refusal" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    expect(await screen.findByTestId("acp-turn-tool-call-9")).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByTestId("acp-tool-status-call-9").textContent).toContain("失败");
    });
    await waitFor(() => {
      expect(screen.getByTestId("acp-transcript").textContent).toContain("模型拒绝回答");
    });
  });
});

describe("AcpView request_permission", () => {
  const PERMISSION_SCRIPT = [
    { kind: "permission", toolKind: "execute", title: "Run tests" },
    { kind: "message", text: "after" },
    { kind: "stop", reason: "end_turn" },
  ] as const;

  it("批准：点 allow 选项回 selected outcome，选项按钮消失显示已批准", async () => {
    mockAcpConsole.configure({ promptScript: [...PERMISSION_SCRIPT] });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    await screen.findByTestId("acp-permission-row-" + id);
    expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("等待处理");
    fireEvent.click(screen.getByTestId("acp-permission-option-" + id + "-allow-once"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已批准");
    });
    expect(screen.queryByTestId("acp-permission-option-" + id + "-allow-once")).toBeNull();
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "selected", optionId: "allow-once" },
    });
  });

  it("拒绝：存在 reject 选项时点档回 selected reject-once", async () => {
    mockAcpConsole.configure({ promptScript: [...PERMISSION_SCRIPT] });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    await screen.findByTestId("acp-permission-row-" + id);
    fireEvent.click(screen.getByTestId("acp-permission-option-" + id + "-reject-once"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已拒绝");
    });
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "selected", optionId: "reject-once" },
    });
  });

  it("倒计时归零：自动按拒绝应答并显示已拒绝（与桥侧 60s 对齐）", async () => {
    mockAcpConsole.configure({ promptScript: [...PERMISSION_SCRIPT] });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    await screen.findByTestId("acp-permission-row-" + id);
    act(() => {
      useAcpStore.setState((s) => ({
        interactions: {
          ...s.interactions,
          "s-001": {
            ...s.interactions["s-001"],
            permissions: s.interactions["s-001"].permissions.map((req) => ({
              ...req,
              receivedAt: req.receivedAt - 61_000,
            })),
          },
        },
      }));
    });
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已拒绝");
    });
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "cancelled" },
    });
    expect(screen.queryByTestId("acp-permission-approve-" + id)).toBeNull();
  });
});

describe("AcpView config options", () => {
  it("下拉与选项来自 agent 真实目录，切换走 set_config_option 并回写目录", async () => {
    await renderConnected();
    await newSession();
    const panel = screen.getByTestId("acp-config-panel");
    expect(panel.textContent).toContain("模型");
    expect(panel.textContent).toContain("思考深度");
    await pickOption("acp-config-option-model", "Mock Model B");
    await waitFor(() => {
      expect(screen.getByTestId("acp-config-option-model").textContent).toContain("Mock Model B");
    });
  });

  it("agent 推送 config_option_update 整表覆盖本地目录", async () => {
    mockAcpConsole.configure({
      promptScript: [
        {
          kind: "config",
          options: [
            {
              id: "model",
              name: "Model",
              category: "model",
              type: "select",
              currentValue: "m2",
              options: [{ value: "m2", name: "M2" }],
            },
          ],
        },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    await waitFor(() => {
      expect(screen.queryByTestId("acp-config-option-thought_level")).toBeNull();
    });
    await waitFor(() => {
      expect(screen.getByTestId("acp-config-option-model").textContent).toContain("M2");
    });
  });
});

describe("AcpView usage bar", () => {
  it("usage_update 渲染上下文占用条（数值+比例）", async () => {
    mockAcpConsole.configure({
      promptScript: [
        { kind: "usage", used: 53000, size: 200000 },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    expect(await screen.findByTestId("acp-usage-bar")).toBeTruthy();
    expect(screen.getByTestId("acp-usage-text").textContent).toContain("53000 / 200000");
    expect(screen.getByTestId("acp-usage-text").textContent).toContain("27%");
    expect((screen.getByTestId("acp-usage-fill") as HTMLElement).style.width).toBe("27%");
  });

  it("无 usage 数据不渲染占用条", async () => {
    await renderConnected();
    await newSession();
    expect(screen.queryByTestId("acp-usage-bar")).toBeNull();
  });
});

describe("AcpView reattach banner", () => {
  it("dsh/bridge/reattach 通知显示已续连补放 N 条", async () => {
    await renderConnected();
    expect(screen.queryByTestId("acp-reattach-banner")).toBeNull();
    act(() => {
      mockAcpConsole.pushReattach(3);
    });
    // 帧解码为异步队列（Blob 路径），横幅在微任务后上墙
    const banner = await screen.findByTestId("acp-reattach-banner");
    expect(banner.textContent).toContain("已续连");
    expect(banner.textContent).toContain("3");
  });
});

describe("AcpView connection directory", () => {
  it("手动 PeerId 添加、scope 只读徽章（不可切换）、回填表单、移除", async () => {
    render(<AcpView />);
    fireEvent.change(screen.getByTestId("acp-directory-input"), {
      target: { value: "peer-manual-1" },
    });
    fireEvent.click(screen.getByTestId("acp-directory-add"));
    const row = screen.getByTestId("acp-directory-row-peer-manual-1");
    expect(row.textContent).toContain("手动");
    expect(screen.getByTestId("acp-directory-scope-badge-peer-manual-1").textContent).toContain("沙箱");
    // P2-ADD：scope 为只读展示（真实授权走桥侧 p2pctl acp allow），切换下拉已删除
    expect(screen.queryByTestId("acp-directory-scope-peer-manual-1")).toBeNull();
    expect(screen.getByTestId("acp-directory-scope-hint").textContent).toContain("p2pctl acp allow");
    fireEvent.click(screen.getByTestId("acp-directory-fill-peer-manual-1"));
    expect((screen.getByTestId("acp-input-peer") as HTMLInputElement).value).toBe("peer-manual-1");
    fireEvent.click(screen.getByTestId("acp-directory-remove-peer-manual-1"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-directory-row-peer-manual-1")).toBeNull();
    });
  });

  it("发现清单经 discovery 契约上墙，发现徽章与地址可见", async () => {
    render(<AcpView />);
    act(() => {
      mockAcpConsole.discoveryPeers = [{ peer: "disc-1", addrs: ["/ip4/10.0.0.8/tcp/4001"] }];
      mockAcpConsole.onDiscovery = (peers) => useAcpStore.getState().ingestDiscovery(peers);
      mockAcpConsole.emitDiscovery();
    });
    const row = screen.getByTestId("acp-directory-row-disc-1");
    expect(row.textContent).toContain("发现");
    expect(row.textContent).toContain("/ip4/10.0.0.8/tcp/4001");
    expect(screen.getByTestId("acp-directory-scope-badge-disc-1").textContent).toContain("沙箱");
  });

  it("空 PeerId 提交显示校验错误", () => {
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-directory-add"));
    expect(screen.getByTestId("acp-directory-error")).toBeTruthy();
  });
});