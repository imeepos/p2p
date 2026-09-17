import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import {
  FormProvider,
  useForm,
  useFormContext,
  type UseFormReturn,
} from "react-hook-form";
import { describe, expect, it } from "vitest";

import "@/i18n";
import type { GuiConfig } from "@/lib/ipc-types";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  toFormValues,
  toGuiConfig,
  type SettingsFormValues,
} from "./config-schema";
import { RemoteAccessCard } from "./remote-access-card";

type FormRef = { current: UseFormReturn<SettingsFormValues> | null };

const BASE_CONFIG: GuiConfig = {
  quicPort: 0,
  tcpPort: 0,
  enableMdns: true,
  dataDir: "",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
  lanOnly: false,
  authzDefaultRole: "friend",
};

// 非缺省值：任何一环丢字段都会在这里显形（config_save 整文件覆写）。
const FULL_CONFIG: GuiConfig = {
  ...BASE_CONFIG,
  rdRequireApproval: false,
  rdFps: 30,
  tunnelServeAllow: ["127.0.0.1:7820", "127.0.0.1:7821"],
};

function DirtyProbe() {
  const { formState } = useFormContext<SettingsFormValues>();
  return (
    <span data-testid="form-dirty">
      {formState.isDirty ? "dirty" : "clean"}
    </span>
  );
}

function Harness({
  values,
  formRef,
  children,
}: {
  values: SettingsFormValues;
  formRef: FormRef;
  children: React.ReactNode;
}) {
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: values,
  });
  useEffect(() => {
    formRef.current = form;
  }, [form, formRef]);
  return (
    <FormProvider {...form}>
      {children}
      <DirtyProbe />
    </FormProvider>
  );
}

async function isFieldValid(
  overrides: Partial<SettingsFormValues>,
  field: keyof SettingsFormValues,
): Promise<boolean> {
  const formRef: FormRef = { current: null };
  render(
    <Harness values={{ ...EMPTY_SETTINGS, ...overrides }} formRef={formRef}>
      <RemoteAccessCard />
    </Harness>,
  );
  let valid = false;
  await act(async () => {
    valid = (await formRef.current?.trigger(field)) === true;
  });
  return valid;
}

describe("remoteAccess 三字段 roundtrip（config_save 整包防丢）", () => {
  it("toFormValues 缺省归一 true/15/[]", () => {
    const values = toFormValues(BASE_CONFIG);
    expect(values.rdRequireApproval).toBe(true);
    expect(values.rdFps).toBe(15);
    expect(values.tunnelServeAllow).toEqual([]);
  });

  it("toFormValues 显式值原样保留", () => {
    const values = toFormValues(FULL_CONFIG);
    expect(values.rdRequireApproval).toBe(false);
    expect(values.rdFps).toBe(30);
    expect(values.tunnelServeAllow).toEqual([
      { value: "127.0.0.1:7820" },
      { value: "127.0.0.1:7821" },
    ]);
  });

  it("toGuiConfig(toFormValues) 往返保真：三字段不丢", () => {
    expect(toGuiConfig(toFormValues(FULL_CONFIG))).toEqual(FULL_CONFIG);
  });
});

describe("rdFps 合法域 1..=60", () => {
  const cases: Array<[SettingsFormValues["rdFps"], boolean]> = [
    [1, true],
    [15, true],
    [60, true],
    [0, false],
    [61, false],
    [15.5, false],
    [Number.NaN, false],
  ];
  it.each(cases)("rdFps=%p 校验通过性 %p", async (fps, expected) => {
    expect(await isFieldValid({ rdFps: fps }, "rdFps")).toBe(expected);
  });
});

describe("tunnelServeAllow 行校验（127.0.0.1:<端口> 字面量）", () => {
  it("合法回环地址通过", async () => {
    expect(
      await isFieldValid(
        { tunnelServeAllow: [{ value: "127.0.0.1:7820" }] },
        "tunnelServeAllow",
      ),
    ).toBe(true);
  });

  it("非回环前缀/越界端口/重复行拒绝", async () => {
    const bad: AddressRowSet[] = [
      [{ value: "192.168.0.2:7820" }],
      [{ value: "127.0.0.1:0" }],
      [{ value: "127.0.0.1:65536" }],
      [{ value: "localhost:7820" }],
      [
        { value: "127.0.0.1:7820" },
        { value: "127.0.0.1:7820" },
      ],
    ];
    for (const rows of bad) {
      expect(
        await isFieldValid({ tunnelServeAllow: rows }, "tunnelServeAllow"),
      ).toBe(false);
    }
  });
});

type AddressRowSet = SettingsFormValues["tunnelServeAllow"];

describe("RemoteAccessCard 控件", () => {
  it("审批开关切换写入表单并置脏", async () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness values={EMPTY_SETTINGS} formRef={formRef}>
        <RemoteAccessCard />
      </Harness>,
    );
    expect(formRef.current?.getValues("rdRequireApproval")).toBe(true);
    fireEvent.click(screen.getByLabelText("新会话需审批"));
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    expect(formRef.current?.getValues("rdRequireApproval")).toBe(false);
  });

  it("帧率输入写入表单并置脏", async () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness values={EMPTY_SETTINGS} formRef={formRef}>
        <RemoteAccessCard />
      </Harness>,
    );
    fireEvent.change(screen.getByLabelText("默认帧率"), {
      target: { value: "45" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    expect(formRef.current?.getValues("rdFps")).toBe(45);
  });

  it("白名单可添加行", () => {
    render(
      <Harness values={EMPTY_SETTINGS} formRef={{ current: null }}>
        <RemoteAccessCard />
      </Harness>,
    );
    fireEvent.click(screen.getByRole("button", { name: "添加地址" }));
    expect(screen.getByLabelText("地址 1")).toBeInTheDocument();
  });

  it("白名单容器带 data-field 标注（focusFirstInvalidField 定位依赖）", () => {
    const { container } = render(
      <Harness values={EMPTY_SETTINGS} formRef={{ current: null }}>
        <RemoteAccessCard />
      </Harness>,
    );
    expect(
      container.querySelector('[data-field="tunnelServeAllow"]'),
    ).not.toBeNull();
  });
});
