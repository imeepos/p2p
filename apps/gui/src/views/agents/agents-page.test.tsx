// /agents 页渲染矩阵（A2A3 验收）：空态/行内动作/创建校验/可见性徽章/编辑
// 占位说明/下架/skills chip 上限与去重。IPC mock 命令名与契约逐字一致；
// admin HTTP 用 fetch stub；WS 通道注入 fake 工厂（不发帧，测通道外 UI 面）。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Toaster, toast } from "sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";

import "@/i18n";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    acpLocalDescriptor: vi.fn(),
    acpConsoleStatus: vi.fn(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    acpLocalDescriptor: mocks.acpLocalDescriptor,
    acpConsoleStatus: mocks.acpConsoleStatus,
  },
}));

import { useAgentsStore } from "@/a2a/agents-store";
import { setWsFactory } from "@/acp/ws-factory";
import type { AgentCardJson, AgentDefJson } from "@/a2a/types";

import { AgentsPage } from "./agents-page";

// sonner Toaster 容器随页挂载（friend-invite-row 先例）：toast 断言依赖其进 DOM
function renderPage(): ReturnType<typeof render> {
  return render(
    <>
      <Toaster />
      <AgentsPage />
    </>,
  );
}

const HOST_PEER = "peer-host-aaaaaaaa";

const card: AgentCardJson = {
  agentId: "code-review",
  name: "代码审查",
  description: "审查代码变更并给出建议",
  url: "a2a://" + HOST_PEER + "/code-review",
  hostPeer: HOST_PEER,
  visibility: "public",
  skills: [{ id: "code-review", name: "代码审查" }],
  ttlSecs: 300,
  version: 3,
};

const mineDef: AgentDefJson = {
  agentId: "writer",
  name: "写作助手",
  description: "润色与扩写",
  skills: [],
  visibility: "private",
  enabled: true,
  createdAt: 1_700_000_000,
};

function descriptor() {
  return { adminUrl: "http://127.0.0.1:9101", token: "admintoken", peer: HOST_PEER, agentName: "agent", writtenAtUnix: 1 };
}

function statusReady() {
  return { phase: "ready", wsUrl: "ws://127.0.0.1:9100", token: "wstoken", restarts: 0 };
}

let fetchCalls: Array<{ url: string; init?: RequestInit }>;

beforeEach(() => {
  vi.clearAllMocks();
  toast.dismiss();
  fetchCalls = [];
  vi.stubGlobal("fetch", vi.fn(async (url: string | URL, init?: RequestInit) => {
    fetchCalls.push({ url: String(url), init });
    const path = String(url);
    const respond = (body: unknown) =>
      ({ ok: true, status: 200, json: async () => body }) as Response;
    if (path.endsWith("/a2a/agents") && (!init?.method || init.method === "GET")) {
      return respond({ agents: [mineDef] });
    }
    if (path.endsWith("/a2a/agents") && init?.method === "POST") {
      return respond({ ...mineDef, agentId: "created-1", name: JSON.parse(String(init.body)).name });
    }
    if (path.endsWith("/a2a/agents/writer") && init?.method === "PUT") {
      return respond({ ...mineDef, visibility: JSON.parse(String(init.body)).visibility });
    }
    if (path.endsWith("/a2a/agents/writer") && init?.method === "DELETE") {
      return respond({ removed: "writer" });
    }
    return { ok: false, status: 404, json: async () => ({ error: "not-found" }) } as Response;
  }));
  setWsFactory(() => ({
    send: vi.fn(),
    close: vi.fn(),
    onopen: null,
    onclose: null,
    onerror: null,
    onmessage: null,
  }));
  mocks.acpLocalDescriptor.mockResolvedValue(descriptor());
  mocks.acpConsoleStatus.mockResolvedValue(statusReady());
  useAgentsStore.setState({
    discovered: [],
    mine: [],
    channelStatus: "offline",
    channelError: null,
    lastActionError: null,
    mineUnavailable: false,
  });
});

describe("AgentsPage", () => {
  it("发现视图空态与双视图切换（我的视图经 admin 拉取到达）", async () => {
    renderPage();
    await waitFor(() => expect(screen.getByTestId("agents-discover-section")).toBeInTheDocument());
    expect(screen.getByTestId("agents-discover-empty")).toHaveTextContent("暂无公开智能体");
    fireEvent.click(screen.getByTestId("segmented-mine"));
    // loadMine 异步到达后显示 admin 面数据行（mock 返回一条定义）
    expect(await screen.findByTestId("agents-mine-row")).toHaveTextContent("写作助手");
    await waitFor(() =>
      expect(fetchCalls.some((c) => c.url.endsWith("/a2a/agents"))).toBe(true),
    );
  });

  it("我的视图空态：admin 不可达降级为空列表（旧 agent 常态）", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: false, status: 404, json: async () => ({ error: "not-found" }) }) as Response));
    renderPage();
    fireEvent.click(await screen.findByTestId("segmented-mine"));
    expect(await screen.findByTestId("agents-mine-empty")).toBeInTheDocument();
  });

  it("发现行渲染：名称/描述/skills/可见性徽章/详情对话框", async () => {
    useAgentsStore.setState({ discovered: [{ card, issuedAtSecs: Math.floor(Date.now() / 1000) }] });
    renderPage();
    const row = await screen.findByTestId("agents-discover-row");
    expect(row).toHaveTextContent("代码审查");
    expect(row).toHaveTextContent("审查代码变更并给出建议");
    expect(screen.getByTestId("agents-visibility-public")).toHaveTextContent("公开");
    fireEvent.click(screen.getByTestId("agents-detail-btn"));
    expect(screen.getByTestId("agents-detail-dialog")).toHaveTextContent("code-review");
  });

  it("聊天按钮为占位：toast 显式说明且不导航", async () => {
    useAgentsStore.setState({ discovered: [{ card, issuedAtSecs: 0 }] });
    renderPage();
    fireEvent.click(await screen.findByTestId("agents-chat-btn"));
    expect(await screen.findByText("聊天链路即将开通（下一版本）")).toBeInTheDocument();
  });

  it("创建对话框：空名称/描述原位上浮；补全后 POST admin", async () => {
    renderPage();
    fireEvent.click(await screen.findByTestId("segmented-mine"));
    fireEvent.click(await screen.findByTestId("agents-create-btn"));
    fireEvent.click(screen.getByTestId("agents-dialog-confirm"));
    expect(await screen.findByTestId("agents-name-error")).toBeInTheDocument();
    expect(screen.getByTestId("agents-desc-error")).toBeInTheDocument();

    fireEvent.change(screen.getByTestId("agents-name-input"), { target: { value: "新智能体" } });
    fireEvent.change(screen.getByTestId("agents-desc-input"), { target: { value: "描述" } });
    fireEvent.click(screen.getByTestId("agents-dialog-confirm"));
    await waitFor(() => expect(screen.getByText("智能体已创建并发布")).toBeInTheDocument());
    const post = fetchCalls.find((c) => c.init?.method === "POST");
    expect(post).toBeDefined();
    const body = JSON.parse(String(post!.init!.body));
    expect(body.visibility).toBe("public");
    expect(body.name).toBe("新智能体");
  });

  it("编辑对话框：v1 仅可见性可改（字段禁用 + PUT 提交）", async () => {
    useAgentsStore.setState({ mine: [mineDef] });
    renderPage();
    fireEvent.click(await screen.findByTestId("segmented-mine"));
    fireEvent.click(screen.getByTestId("agents-edit-btn"));
    expect(screen.getByTestId("agents-edit-hint")).toBeInTheDocument();
    expect(screen.getByTestId("agents-name-input")).toBeDisabled();
    fireEvent.click(screen.getByTestId("segmented-public"));
    fireEvent.click(screen.getByTestId("agents-dialog-confirm"));
    await waitFor(() => expect(screen.getByText("智能体已更新")).toBeInTheDocument());
    const put = fetchCalls.find((c) => c.init?.method === "PUT");
    expect(put).toBeDefined();
    expect(JSON.parse(String(put!.init!.body)).visibility).toBe("public");
  });

  it("下架动作：DELETE admin 且成功 toast", async () => {
    useAgentsStore.setState({ mine: [mineDef] });
    renderPage();
    fireEvent.click(await screen.findByTestId("segmented-mine"));
    fireEvent.click(screen.getByTestId("agents-unpublish-btn"));
    await waitFor(() => expect(screen.getByText("智能体已下架")).toBeInTheDocument());
    expect(fetchCalls.some((c) => c.init?.method === "DELETE")).toBe(true);
  });

  it("skills chip：回车成 chip、重复去重、超 10 上限提示", async () => {
    renderPage();
    fireEvent.click(await screen.findByTestId("segmented-mine"));
    fireEvent.click(screen.getByTestId("agents-create-btn"));
    const input = screen.getByTestId("agents-skill-input");
    fireEvent.change(input, { target: { value: "翻译" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.change(input, { target: { value: "翻译" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const chips = screen.getByTestId("agents-skills-chips");
    expect(chips).toHaveTextContent("翻译");
    expect(screen.getAllByTestId("agents-skill-remove-翻译").length).toBe(1);
    fireEvent.change(input, { target: { value: "s1" } });
    fireEvent.keyDown(input, { key: "Enter" });
    for (let i = 2; i <= 10; i += 1) {
      fireEvent.change(input, { target: { value: "s" + i } });
      fireEvent.keyDown(input, { key: "Enter" });
    }
    expect(screen.getByText("技能最多 10 条")).toBeInTheDocument();
  });
});
