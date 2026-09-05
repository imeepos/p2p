import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useCallback, useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { beforeEach, describe, expect, it, vi } from "vitest";

const configSaveMock = vi.fn(
  async (cfg?: Record<string, unknown>) => cfg ?? {},
);
vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: vi.fn(async () => ({
      quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp",
      bootstrap: [], relayAddrs: [], advertisedAddrs: [],
      observationPort: null, observationAddrs: [],
    })),
    configSave: (...args: unknown[]) => configSaveMock(...(args as [])),
  },
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { NetworkCard } from "./network-card";
import { SettingsSaveBar } from "./save-bar";
import { focusFirstInvalidField } from "./focus-first-error";
import { useSettingsSave } from "./use-settings-save";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  type SettingsFormValues,
} from "./config-schema";

// 与 settings-view 相同的保存组合：卡表单 + 保存条 + 校验反馈状态
function SaveHarness() {
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: EMPTY_SETTINGS,
  });
  const [invalidCount, setInvalidCount] = useState(0);
  const reportInvalid = useCallback((count: number) => setInvalidCount(count), []);
  const { submitSave, saveAndRestart } = useSettingsSave(form, reportInvalid);
  const requestSave = useCallback(async () => {
    setInvalidCount(0);
    await submitSave();
  }, [submitSave]);
  return (
    <FormProvider {...form}>
      <NetworkCard />
      <SettingsSaveBar
        dirty={form.formState.isDirty}
        loaded
        running={false}
        invalidCount={invalidCount}
        onSubmit={requestSave}
        onSaveAndRestart={saveAndRestart}
        onReportSaveError={() => {}}
        onReportRestartError={() => {}}
      />
    </FormProvider>
  );
}

beforeEach(() => {
  configSaveMock.mockClear();
});

// useConfirm 在 SaveHarness 内调用：Provider 必须包住 hook 调用方本身
function renderHarness() {
  return render(
    <ConfirmProvider>
      <SaveHarness />
    </ConfirmProvider>,
  );
}

describe("focusFirstInvalidField", () => {
  it("按字段顺序聚焦第一个错误字段并触发滚动", () => {
    document.body.innerHTML =
      '<input id="settings-quic-port" /><input id="settings-tcp-port" />';
    const scrollSpy = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollSpy;
    const errors = {
      quicPort: { type: "portRange", message: "portRange" },
      tcpPort: { type: "portRange", message: "portRange" },
    } as never;
    const count = focusFirstInvalidField(errors);
    expect(count).toBe(2);
    expect(document.activeElement?.id).toBe("settings-quic-port");
    expect(scrollSpy).toHaveBeenCalledTimes(1);
  });

  it("地址列表字段经 data-field 容器定位行输入", () => {
    document.body.innerHTML =
      '<div data-field="advertisedAddrs"><input placeholder="addr" /></div>';
    HTMLElement.prototype.scrollIntoView = vi.fn();
    const errors = {
      advertisedAddrs: { root: { type: "addrDuplicate", message: "addrDuplicate" } },
    } as never;
    focusFirstInvalidField(errors);
    expect(document.activeElement?.getAttribute("placeholder")).toBe("addr");
  });
});

describe("设置保存校验失败可见反馈", () => {
  it("非法端口保存：保存条出现汇总提示，聚焦并滚动到首个错误字段，不落盘", async () => {
    const scrollSpy = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollSpy;
    renderHarness();
    const quic = document.getElementById("settings-quic-port") as HTMLInputElement;
    fireEvent.change(quic, { target: { value: "99999" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("未通过校验");
    await waitFor(() => {
      expect(document.activeElement?.id).toBe("settings-quic-port");
    });
    expect(scrollSpy).toHaveBeenCalled();
    expect(configSaveMock).not.toHaveBeenCalled();
  });

  it("修正后再次保存成功：提示消失且 configSave 收到新值", async () => {
    HTMLElement.prototype.scrollIntoView = vi.fn();
    renderHarness();
    const quic = document.getElementById("settings-quic-port") as HTMLInputElement;
    fireEvent.change(quic, { target: { value: "99999" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByRole("alert");
    // AsyncButton fail 态驻留后回 idle 才可再次点击
    const saveAgain = (await screen.findByRole("button", {
      name: "保存",
    })) as HTMLButtonElement;
    await waitFor(() => expect(saveAgain.disabled).toBe(false), {
      timeout: 3000,
    });
    fireEvent.change(quic, { target: { value: "3400" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => {
      expect(configSaveMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.queryByRole("alert")).toBeNull();
    });
    expect(configSaveMock.mock.calls[0][0]).toMatchObject({ quicPort: 3400 });
  });
});
