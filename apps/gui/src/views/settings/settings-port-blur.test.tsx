import { act, fireEvent, render, screen } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it } from "vitest";

import "@/i18n";

import { NetworkCard } from "./network-card";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  type SettingsFormValues,
} from "./config-schema";

function Harness() {
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: EMPTY_SETTINGS,
  });
  return (
    <FormProvider {...form}>
      <NetworkCard />
    </FormProvider>
  );
}

// 终验 F14 复盘：headless Chrome 里程序化 focus()+blur() 对 React 派发不出
// 任何焦点事件（UX-J known-issues），走查端因此误报「失焦无校验」。单测必须
// 以真实交互同款 focus-then-blur 事件对断言（jsdom 下 blur() 冒泡 focusout，
// 与用户点击别处走同一条 React 委托路径），裸 fireEvent.blur 不再作失焦口径。
async function blur(input: HTMLElement) {
  await act(async () => {
    input.focus();
    input.blur();
  });
}

// F14：数值/范围字段失焦即时校验——越界值失焦提示、合法值不提示、
// 修正即消；口径（role=alert + aria-invalid + describedby）同 dial-target-field。
describe("设置页端口失焦即时校验（F14）", () => {
  it("99999 失焦立即出现 role=alert 提示且 aria-invalid", async () => {
    render(<Harness />);
    const quic = screen.getByLabelText("QUIC 端口") as HTMLInputElement;
    fireEvent.change(quic, { target: { value: "99999" } });
    expect(screen.queryByRole("alert")).toBeNull();
    await blur(quic);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("端口需为 0-65535");
    expect(alert.id).toBe("settings-quic-port-error");
    expect(quic.getAttribute("aria-invalid")).toBe("true");
    expect(quic.getAttribute("aria-describedby")).toBe(
      "settings-quic-port-error",
    );
  });

  it("合法端口失焦不出现提示", async () => {
    render(<Harness />);
    const quic = screen.getByLabelText("QUIC 端口");
    fireEvent.change(quic, { target: { value: "3400" } });
    await blur(quic);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(quic.getAttribute("aria-invalid")).toBeNull();
  });

  it("空值（随机端口语义）失焦不出现提示", async () => {
    render(<Harness />);
    const tcp = screen.getByLabelText("TCP 端口");
    await blur(tcp);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(tcp.getAttribute("aria-invalid")).toBeNull();
  });

  it("提示出现后修正为合法值，提示即消失", async () => {
    render(<Harness />);
    const tcp = screen.getByLabelText("TCP 端口") as HTMLInputElement;
    fireEvent.change(tcp, { target: { value: "99999" } });
    await blur(tcp);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    fireEvent.change(tcp, { target: { value: "3401" } });
    await act(async () => {});
    expect(screen.queryByRole("alert")).toBeNull();
    expect(tcp.getAttribute("aria-invalid")).toBeNull();
  });
});
