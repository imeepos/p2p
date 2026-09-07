import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { configGetMock, configSaveMock, profileGetMock } = vi.hoisted(() => ({
  configGetMock: vi.fn(),
  configSaveMock: vi.fn(),
  profileGetMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: configGetMock,
    configSave: configSaveMock,
    profileGet: profileGetMock,
  },
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { ThemeProvider } from "@/theme/theme-provider";
import { SettingsView } from "./settings-view";

const CONFIG = {
  quicPort: 3400,
  tcpPort: 3401,
  enableMdns: true,
  dataDir: "/tmp",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

// 分节容器全量常挂：通用分节依赖 ThemeProvider，入口行依赖 Router 上下文。
function renderView() {
  return render(
    <MemoryRouter initialEntries={["/settings"]}>
      <ThemeProvider>
        <ConfirmProvider>
          <SettingsView />
        </ConfirmProvider>
      </ThemeProvider>
    </MemoryRouter>,
  );
}

function section(id: string): HTMLElement {
  const element = document.querySelector(
    `[data-testid="settings-section-${id}"]`,
  );
  if (element === null) throw new Error(`section ${id} not mounted`);
  return element as HTMLElement;
}

// 分节显隐契约：hidden 容器保持挂载（表单值/资料草稿切签不丢），
// 校验失败时自动切到网络分节再聚焦，保证保存反馈可见可定位。
describe("设置页分节导航（微信式双栏）", () => {
  beforeEach(() => {
    configGetMock.mockReset().mockResolvedValue(CONFIG);
    configSaveMock.mockReset();
    profileGetMock
      .mockReset()
      .mockResolvedValue({ name: "", description: "", avatar: null });
  });

  it("默认落在账号分节，其余分节隐藏但保持挂载", async () => {
    renderView();
    await waitFor(() => expect(configGetMock).toHaveBeenCalled());
    expect(section("account").className).not.toContain("hidden");
    expect(section("network").className).toContain("hidden");
    expect(section("general")).toBeInTheDocument();
    expect(section("about")).toBeInTheDocument();
  });

  it("点击左栏导航切换分节显隐", async () => {
    renderView();
    await waitFor(() => expect(configGetMock).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId("settings-nav-network"));
    expect(section("network").className).not.toContain("hidden");
    expect(section("account").className).toContain("hidden");
    fireEvent.click(screen.getByTestId("settings-nav-about"));
    expect(section("about").className).not.toContain("hidden");
    expect(section("network").className).toContain("hidden");
  });

  it("校验失败自动切到网络分节且不落盘", async () => {
    // jsdom 无 scrollIntoView：聚焦首错误字段的定位链必须可执行
    HTMLElement.prototype.scrollIntoView = vi.fn();
    renderView();
    // 等异步 config_get 的 form.reset 应用（输入回显 3400），避免改值被重置竞态清掉
    await waitFor(() => {
      const el = document.getElementById(
        "settings-quic-port",
      ) as HTMLInputElement | null;
      if (el === null || el.value !== "3400") {
        throw new Error("config not loaded yet");
      }
    });
    const quic = document.getElementById(
      "settings-quic-port",
    ) as HTMLInputElement;
    fireEvent.change(quic, { target: { value: "99999" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => {
      expect(section("network").className).not.toContain("hidden");
    });
    // 字段级 alert 与保存条汇总并存，取保存条汇总断言
    const alerts = await screen.findAllByRole("alert");
    expect(alerts.some((el) => el.textContent.includes("未通过校验"))).toBe(true);
    expect(configSaveMock).not.toHaveBeenCalled();
  });
});
