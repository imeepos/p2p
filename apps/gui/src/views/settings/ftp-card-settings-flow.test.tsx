import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

const { ftpConfigSaveMock } = vi.hoisted(() => ({
  ftpConfigSaveMock: vi.fn(async () => true),
}));

// SettingsView 挂载即双读（config+ftp）并挂服务区三卡，mock 面必须齐。
vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: vi.fn(async () => ({
      quicPort: 3400,
      tcpPort: 3401,
      enableMdns: true,
      dataDir: "/tmp",
      bootstrap: [],
      relayAddrs: [],
      advertisedAddrs: [],
      observationPort: null,
      observationAddrs: [],
    })),
    configSave: vi.fn(async (cfg?: unknown) => cfg),
    profileGet: vi.fn(async () => ({ name: "", description: "", avatar: null })),
    servicesList: vi.fn(async () => ({ services: [] })),
    ftpConfigGet: vi.fn(async () => ({ root: "/srv/ftp", authz: true, users: ["alice"] })),
    ftpConfigSave: (...args: unknown[]) => ftpConfigSaveMock(...(args as [])),
    staticPeersList: vi.fn(async () => ({ peers: [] })),
    staticPeersUpsert: vi.fn(async () => true),
    staticPeersRemove: vi.fn(async () => true),
  },
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { ThemeProvider } from "@/theme/theme-provider";
import {
  discardAllUnsaved,
  hasAnyUnsaved,
} from "@/views/shared/use-unsaved-guard";
import { SettingsView } from "./settings-view";

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

async function waitFtpLoaded(): Promise<HTMLInputElement> {
  await waitFor(() => {
    const el = document.getElementById(
      "settings-ftp-root",
    ) as HTMLInputElement | null;
    if (el === null || el.value !== "/srv/ftp") {
      throw new Error("ftp config not loaded yet");
    }
  });
  return document.getElementById("settings-ftp-root") as HTMLInputElement;
}

describe("设置页集成：FTP 卡走既有保存/放弃流程", () => {
  it("编辑根目录后保存：configSave 与 ftpConfigSave 双写，脏状态归零", async () => {
    HTMLElement.prototype.scrollIntoView = vi.fn();
    renderView();
    const root = await waitFtpLoaded();
    fireEvent.change(root, { target: { value: "/srv/ftp2" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => {
      expect(ftpConfigSaveMock).toHaveBeenCalledWith(
        "/srv/ftp2",
        true,
        { alice: "" }, // 既有用户未改密码 → 空串=保留原密码（钉死语义）
      );
    });
    // 保存后回读重置：保存条回到「配置已与磁盘一致」
    await waitFor(() => {
      expect(screen.getByText("配置已与磁盘一致")).toBeInTheDocument();
    });
  });

  it("脏草稿登记守卫；放弃后回滚 FTP 编辑", async () => {
    renderView();
    const root = await waitFtpLoaded();
    fireEvent.change(root, { target: { value: "/draft" } });
    await waitFor(() => expect(hasAnyUnsaved()).toBe(true));
    discardAllUnsaved();
    await waitFor(() => {
      expect(
        (document.getElementById("settings-ftp-root") as HTMLInputElement).value,
      ).toBe("/srv/ftp");
    });
    expect(hasAnyUnsaved()).toBe(false);
  });
});