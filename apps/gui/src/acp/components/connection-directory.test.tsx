// 连接目录信息架构组件测试（P2 页面打磨）：发现/手动分组、空态引导、一键回填。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import type { DirectoryEntry } from "@/acp/directory-model";
import { ConnectionDirectory } from "./connection-directory";

await import("@/i18n");

function entry(over: Partial<DirectoryEntry>): DirectoryEntry {
  return { peer: "peer-x", name: null, scope: "sandbox", source: "manual", addrs: [], ...over };
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
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
});
