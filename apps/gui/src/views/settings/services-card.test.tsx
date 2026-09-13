import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listMock = vi.fn(async () => ({ services: fullServiceList() }));
let deferred: ((value: unknown) => void) | null = null;
const setEnabledMock = vi.fn(
  (serviceId: string, enabled: boolean) =>
    new Promise((resolve) => {
      deferred = (value) =>
        resolve({
          serviceId,
          enabled,
          requiresRestart: false,
          ...(value as object),
        });
    }),
);

vi.mock("@/lib/ipc", () => ({
  ipc: {
    servicesList: () => listMock(),
    servicesSetEnabled: (serviceId: string, enabled: boolean) =>
      setEnabledMock(serviceId, enabled),
  },
}));

import "@/i18n";
import type { ServiceMutationReport } from "@/lib/ipc-types";
import { fullServiceList } from "./services-card-fixtures";
import { ServicesCard } from "./services-card";

function settle(report: Partial<ServiceMutationReport>): void {
  deferred?.(report);
  deferred = null;
}

beforeEach(() => {
  listMock.mockClear();
  setEnabledMock.mockClear();
  listMock.mockImplementation(async () => ({ services: fullServiceList() }));
  setEnabledMock.mockImplementation(
    (serviceId: string, enabled: boolean) =>
      new Promise((resolve) => {
        deferred = (value) =>
          resolve({
            serviceId,
            enabled,
            requiresRestart: false,
            ...(value as object),
          });
      }),
  );
  deferred = null;
});

describe("服务总控卡（gui-contract §20）", () => {
  it("渲染 services_list 全量 10 行：名称/型别/状态/开关", async () => {
    render(<ServicesCard />);
    expect(await screen.findByText("LLM 借出服务")).toBeTruthy();
    expect(screen.getAllByRole("switch")).toHaveLength(10);
    expect(screen.getByTestId("service-kind-serve.llm_share").textContent).toBe(
      "布尔闸",
    );
    expect(screen.getByTestId("service-kind-net.lan_only").textContent).toBe(
      "收编",
    );
    expect(screen.getByText("仅局域网模式")).toBeTruthy();
    expect(screen.getAllByText("已开启").length).toBeGreaterThan(0);
    expect(screen.getAllByText("已关闭").length).toBeGreaterThan(0);
  });

  it("翻转：乐观更新立即可见，成功后落报告值", async () => {
    render(<ServicesCard />);
    const sw = (await screen.findByTestId(
      "service-switch-serve.tunnel",
    )) as HTMLInputElement;
    fireEvent.click(sw);
    // 乐观更新：promise 未决时开关已翻
    expect(setEnabledMock).toHaveBeenCalledWith("serve.tunnel", true);
    expect(sw.getAttribute("data-state")).toBe("checked");
    settle({});
    await waitFor(() => expect(deferred).toBeNull());
    expect(sw.getAttribute("data-state")).toBe("checked");
  });

  it("requiresRestart=true 时顶部显示重启提示条", async () => {
    listMock.mockImplementation(async () => ({
      services: fullServiceList({ requiresRestart: true }),
    }));
    render(<ServicesCard />);
    expect(
      await screen.findByTestId("services-restart-hint"),
    ).toHaveTextContent("重启节点后生效");
  });

  it("翻转成功且 requiresRestart=true 保留提示条（节点运行中）", async () => {
    listMock.mockImplementation(async () => ({
      services: fullServiceList({ requiresRestart: true }),
    }));
    render(<ServicesCard />);
    await screen.findByTestId("services-restart-hint");
    fireEvent.click(screen.getByTestId("service-switch-serve.llm_share"));
    settle({ requiresRestart: true });
    await waitFor(() => expect(deferred).toBeNull());
    expect(screen.getByTestId("services-restart-hint")).toBeTruthy();
  });

  it("翻转失败：回滚乐观更新并保留原状态", async () => {
    render(<ServicesCard />);
    const sw = (await screen.findByTestId(
      "service-switch-serve.llm_share",
    )) as HTMLInputElement;
    setEnabledMock.mockImplementation(async () => {
      throw new Error("services.json 损坏或版本不符，服务总控已拒绝读取");
    });
    fireEvent.click(sw);
    await waitFor(() =>
      expect(sw.getAttribute("data-state")).toBe("unchecked"),
    );
  });

  it("清单加载失败进入错误态（不白屏），重试恢复", async () => {
    listMock.mockImplementation(async () => {
      throw new Error("services.json 损坏或版本不符");
    });
    render(<ServicesCard />);
    expect(await screen.findByTestId("services-load-failed")).toBeTruthy();
    listMock.mockImplementation(async () => ({ services: fullServiceList() }));
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    expect(await screen.findByText("LLM 借出服务")).toBeTruthy();
    expect(screen.getAllByRole("switch")).toHaveLength(10);
  });
});
