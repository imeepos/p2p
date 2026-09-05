// 连接目录信息架构组件测试（P2 页面打磨）：发现/手动分组、空态引导、一键回填。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import type { DirectoryEntry } from "@/acp/directory-model";
import { ConnectionDirectory } from "./connection-directory";

await import("@/i18n");

function entry(over: Partial<DirectoryEntry>): DirectoryEntry {
  return {
    peer: "peer-x",
    name: null,
    scope: "sandbox",
    source: "manual",
    addrs: [],
    via: null,
    ...over,
  };
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("ConnectionDirectory 信息架构", () => {
  it("发现与手动分两组呈现，条目含名字/地址/来源与 scope 徽章", () => {
    useAcpStore.setState({
      directory: [
        entry({ peer: "d-1", source: "discovered", name: "edge-node", addrs: ["/ip4/10.0.0.8/tcp/4001"] }),
        entry({ peer: "m-1", source: "manual" }),
      ],
    });
    render(<ConnectionDirectory />);
    const discovered = screen.getByTestId("acp-directory-group-discovered");
    const manual = screen.getByTestId("acp-directory-group-manual");
    const dRow = screen.getByTestId("acp-directory-row-d-1");
    expect(discovered.contains(dRow)).toBe(true);
    expect(manual.contains(screen.getByTestId("acp-directory-row-m-1"))).toBe(true);
    expect(dRow.textContent).toContain("edge-node");
    expect(dRow.textContent).toContain("/ip4/10.0.0.8/tcp/4001");
    expect(dRow.textContent).toContain("发现");
    expect(dRow.textContent).toContain("沙箱");
    expect(manual.textContent).toContain("手动 / 已保存");
  });

  it("目录空态展示出现引导（rendezvous / mDNS / 手动添加）", () => {
    useAcpStore.setState({ directory: [] });
    render(<ConnectionDirectory />);
    expect(screen.getByText("暂无节点，发现或手动添加后出现在这里")).toBeTruthy();
    expect(screen.getByText(/rendezvous/)).toBeTruthy();
    expect(screen.getByText(/mDNS/)).toBeTruthy();
  });

  it("条目一键回填连接表单（draft.peer 写回）", () => {
    useAcpStore.setState({ directory: [entry({ peer: "m-9" })] });
    render(<ConnectionDirectory />);
    fireEvent.click(screen.getByTestId("acp-directory-fill-m-9"));
    expect(useAcpStore.getState().draft.peer).toBe("m-9");
  });

  it("scope 只读：下拉已删、徽章 title 与旁注指向 p2pctl acp allow（P2-ADD 需求9）", () => {
    useAcpStore.setState({ directory: [entry({ peer: "m-1" })] });
    render(<ConnectionDirectory />);
    expect(screen.queryByTestId("acp-directory-scope-m-1")).toBeNull();
    const badge = screen.getByTestId("acp-directory-scope-badge-m-1");
    expect(badge.querySelector("span[title]")?.getAttribute("title")).toContain("p2pctl acp allow");
    expect(screen.getByTestId("acp-directory-scope-hint").textContent).toContain("p2pctl acp allow");
  });

  it("手动添加输入框带 aria-label（可访问名）", () => {
    render(<ConnectionDirectory />);
    expect(screen.getByTestId("acp-directory-input").getAttribute("aria-label")).toBe(
      "手动添加 Peer ID",
    );
  });

  it("发现条目渲染来源渠道徽章（via，目标二）", () => {
    useAcpStore.setState({
      directory: [entry({ peer: "d-mdns", source: "discovered", via: "mdns", name: "edge" })],
    });
    render(<ConnectionDirectory />);
    expect(screen.getByTestId("acp-directory-via-d-mdns").textContent).toBe("mdns");
    const row = screen.getByTestId("acp-directory-row-d-mdns");
    expect(row.textContent).toContain("edge");
  });

  it("/discovery 轮询去重合并：同 peer 只一行且地址并集（目标二）", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({
          peers: [
            {
              peer: "peer-x",
              addrs: ["/ip4/10.0.0.9/tcp/4001"],
              name: "renamed",
              source: "rendezvous",
            },
          ],
        }),
      })),
    );
    useAcpStore.setState({
      draft: {
        wsUrl: "ws://127.0.0.1:1",
        token: "t",
        peer: "p",
        statusUrl: "http://127.0.0.1:1",
      },
      directory: [
        entry({ peer: "peer-x", source: "discovered", addrs: ["/ip4/10.0.0.8/tcp/4001"] }),
      ],
    });
    render(<ConnectionDirectory />);
    await waitFor(() => {
      expect(useAcpStore.getState().directory[0].addrs).toEqual([
        "/ip4/10.0.0.8/tcp/4001",
        "/ip4/10.0.0.9/tcp/4001",
      ]);
    });
    expect(useAcpStore.getState().directory).toHaveLength(1);
    expect(useAcpStore.getState().directory[0]).toMatchObject({ name: "renamed", via: "rendezvous" });
    expect(screen.getByTestId("acp-directory-via-peer-x").textContent).toBe("rendezvous");
  });

  it("console 不可达/无发现：目录空态正常引导，不误报错误（目标二）", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: false, json: async () => ({ error: "unauthorized" }) })),
    );
    useAcpStore.setState({
      draft: {
        wsUrl: "ws://127.0.0.1:1",
        token: "t",
        peer: "p",
        statusUrl: "http://127.0.0.1:1",
      },
      directory: [],
    });
    render(<ConnectionDirectory />);
    await waitFor(() => {
      expect(vi.mocked(fetch).mock.calls.length).toBeGreaterThan(0);
    });
    expect(screen.getByText("暂无节点，发现或手动添加后出现在这里")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(useAcpStore.getState().directory).toHaveLength(0);
  });
});
