import { act, fireEvent, render, screen } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it } from "vitest";

import "@/i18n";

import { toRows } from "@/views/shared/address-rules";
import { AdvertiseCard } from "./advertise-card";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  type SettingsFormValues,
} from "./config-schema";

function Harness({ values }: { values: SettingsFormValues }) {
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: values,
  });
  return (
    <FormProvider {...form}>
      <AdvertiseCard />
    </FormProvider>
  );
}

const withRows = (): SettingsFormValues => ({
  ...EMPTY_SETTINGS,
  advertisedAddrs: toRows(["203.0.113.5/u3400"]),
  observationAddrs: toRows(["203.0.113.5:3402"]),
});

// F13：宣告/观测地址行带可见序号标签；F14：观测端口失焦即时校验。
describe("AdvertiseCard（F13/F14）", () => {
  it("宣告与观测地址行各有可见序号标签并关联输入", () => {
    render(<Harness values={withRows()} />);
    expect(screen.getByText("宣告地址")).toBeInTheDocument();
    expect(screen.getByText("观测地址")).toBeInTheDocument();
    const rowInputs = screen.getAllByLabelText("地址 1");
    expect(rowInputs).toHaveLength(2);
    expect(rowInputs[0]).toHaveValue("203.0.113.5/u3400");
    expect(rowInputs[1]).toHaveValue("203.0.113.5:3402");
  });

  it("观测端口 99999 失焦即时提示 role=alert，合法值不提示", async () => {
    render(<Harness values={EMPTY_SETTINGS} />);
    const port = screen.getByLabelText("观测端口") as HTMLInputElement;
    fireEvent.change(port, { target: { value: "99999" } });
    expect(screen.queryByRole("alert")).toBeNull();
    // 真实交互同款 focus-then-blur 事件对（headless 程序化 blur 无事件，UX-J）
    await act(async () => {
      port.focus();
      port.blur();
    });
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("观测端口需为 1-65535");
    expect(port.getAttribute("aria-invalid")).toBe("true");
    fireEvent.change(port, { target: { value: "3402" } });
    await act(async () => {});
    expect(screen.queryByRole("alert")).toBeNull();
    expect(port.getAttribute("aria-invalid")).toBeNull();
  });
});
