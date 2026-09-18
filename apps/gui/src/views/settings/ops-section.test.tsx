import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import i18n from "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import { OpsSection } from "./ops-section";

function Landed() {
  const { pathname } = useLocation();
  return <div data-testid="landed">{pathname}</div>;
}

function renderOps() {
  return render(
    <MemoryRouter initialEntries={["/settings"]}>
      <Routes>
        <Route path="/settings" element={<OpsSection />} />
        <Route path="*" element={<Landed />} />
      </Routes>
    </MemoryRouter>,
  );
}

// 2026-09-18 一级入口口径拍板：配置辅助入口统一收敛设置页运维区。
// 五入口标题/描述复用各页既有 i18n 键，打开动作可导航到对应路由。
describe("设置页运维区入口（rail 收敛承接）", () => {
  afterEach(() => {
    cleanup();
    useChatStore.setState({ invites: [], groupInvites: [] });
  });

  it("五个配置辅助入口齐备：网络监控/消息中心/远程访问/ACP 管理/协议文档", () => {
    renderOps();
    for (const key of [
      "network.title",
      "messages.title",
      "remoteAccess.title",
      "acpManage.title",
      "docs.title",
    ] as const) {
      expect(screen.getByText(i18n.t(key))).toBeInTheDocument();
    }
  });

  it("点击打开导航到对应路由（首个入口 = /network）", () => {
    renderOps();
    fireEvent.click(screen.getAllByRole("button", { name: "打开" })[0]);
    expect(screen.getByTestId("landed").textContent).toBe("/network");
  });

  it("消息中心行待处理邀请徽标：两类 in 向 pending 之和；归零后消失", () => {
    useChatStore.setState({
      invites: [
        { peerId: "p-in", nickname: "甲", addrs: [], note: null, direction: "in", tsMs: 1, delivered: true },
        { peerId: "p-out", nickname: "丙", addrs: [], note: null, direction: "out", tsMs: 3, delivered: true },
      ],
      groupInvites: [
        { id: "gi-1", groupId: "g-1", groupName: "群", owner: "o", inviter: "i", invitee: "self", note: null, direction: "in", state: "pending", tsMs: 1, delivered: true },
      ],
    });
    renderOps();
    const badge = screen.getByTestId("ops-badge-/messages");
    expect(badge.textContent).toBe("2");
    expect(badge.getAttribute("aria-label")).toBe(
      i18n.t("messages.badgeAria", { count: 2 }),
    );
    cleanup();

    useChatStore.setState({ invites: [], groupInvites: [] });
    renderOps();
    expect(screen.queryByTestId("ops-badge-/messages")).toBeNull();
  });
});
