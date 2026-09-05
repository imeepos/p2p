import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const configGetMock = vi.fn(async () => ({
  quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp",
  bootstrap: [], relayAddrs: [], advertisedAddrs: [],
  observationPort: null, observationAddrs: [],
}));
vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: () => configGetMock(),
    configSave: vi.fn(async (cfg: unknown) => cfg),
    profileGet: vi.fn(async () => ({ name: "", description: "", avatar: null })),
    profileSave: vi.fn(async (p: unknown) => p),
  },
}));
// SettingsView 含 AppearanceCard（useTheme），本文件不验证主题，直接 mock
vi.mock("@/theme/theme-provider", () => ({
  useTheme: () => ({
    theme: "system",
    resolvedTheme: "light",
    setTheme: vi.fn(),
  }),
  ThemeProvider: ({ children }: { children: React.ReactNode }) => children,
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useProfileStore } from "@/stores/profile-store";
import {
  discardAllUnsaved,
  hasAnyUnsaved,
} from "./use-unsaved-guard";
import { ProfileCard } from "@/views/settings/profile-card";
import { RelayConfigCard } from "@/views/relay/relay-config-card";
import { SettingsView } from "@/views/settings/settings-view";

// 需求 1 注册侧：三个编辑面（设置表单 / 节点资料草稿 / 中继地址列表）
// 各自真实挂载后，脏状态进注册表、discard 全部还原。
describe("编辑面路由守卫注册", () => {
  beforeEach(() => {
    useProfileStore.setState({
      profile: { name: "", description: "", avatar: null },
      loaded: true,
      loadError: null,
    });
  });

  it("设置表单：改端口置脏，discardAllUnsaved 还原为磁盘值", async () => {
    render(
      <ConfirmProvider>
        <MemoryRouter>
          <SettingsView />
        </MemoryRouter>
      </ConfirmProvider>,
    );
    await screen.findByText("局域网发现（mDNS）");
    const quic = document.getElementById("settings-quic-port") as HTMLInputElement;
    fireEvent.change(quic, { target: { value: "3401" } });
    expect(hasAnyUnsaved()).toBe(true);
    act(() => {
      discardAllUnsaved();
    });
    await act(async () => {});
    expect(hasAnyUnsaved()).toBe(false);
  });

  it("节点资料草稿：改名称置脏，discard 后丢弃草稿", () => {
    render(
      <ConfirmProvider>
        <ProfileCard />
      </ConfirmProvider>,
    );
    fireEvent.change(screen.getByLabelText("节点名称"), {
      target: { value: "新名字" },
    });
    expect(hasAnyUnsaved()).toBe(true);
    act(() => {
      discardAllUnsaved();
    });
    expect(hasAnyUnsaved()).toBe(false);
    expect(
      (screen.getByLabelText("节点名称") as HTMLInputElement).value,
    ).toBe("");
  });

  it("中继地址列表：编辑行置脏，discard 后还原为传入列表", async () => {
    render(
      <ConfirmProvider>
        <RelayConfigCard
          relayAddrs={["/ip4/10.0.0.2/udp/3403"]}
          onSave={async () => {}}
        />
      </ConfirmProvider>,
    );
    const row = screen.getByPlaceholderText("192.168.1.10/u3403");
    fireEvent.change(row, { target: { value: "bogus" } });
    expect(hasAnyUnsaved()).toBe(true);
    act(() => {
      discardAllUnsaved();
    });
    await waitFor(() => {
      expect(hasAnyUnsaved()).toBe(false);
      expect(
        (screen.getByPlaceholderText("192.168.1.10/u3403") as HTMLInputElement)
          .value,
      ).toBe("/ip4/10.0.0.2/udp/3403");
    });
  });
});
