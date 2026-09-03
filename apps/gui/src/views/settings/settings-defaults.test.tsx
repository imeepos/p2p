import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import {
  FormProvider,
  useForm,
  useFormContext,
  type UseFormReturn,
} from "react-hook-form";
import { afterEach, describe, expect, it } from "vitest";

import "@/i18n";
import type { NodeStatus } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { AdvertiseCard } from "./advertise-card";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  type SettingsFormValues,
} from "./config-schema";
import { NetworkCard } from "./network-card";

type FormRef = { current: UseFormReturn<SettingsFormValues> | null };

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
  // RTL render/fireEvent 包裹 act，effect 提交即刷，后续断言可同步读 ref。
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

function values(
  overrides: Partial<SettingsFormValues> = {},
): SettingsFormValues {
  return { ...EMPTY_SETTINGS, ...overrides };
}

function runningStatus(listenAddrs: string[]): { status: NodeStatus } {
  return {
    status: {
      running: true,
      peerId: null,
      listenAddrs,
      uptimeSecs: 0,
      startedAtMs: null,
      config: {
        quicPort: 0,
        tcpPort: 0,
        enableMdns: true,
        dataDir: "",
        bootstrap: [],
        relayAddrs: [],
        advertisedAddrs: [],
        observationPort: null,
        observationAddrs: [],
      },
    },
  };
}

afterEach(() => {
  useNodeStore.setState({ status: null });
});

describe("settings defaults display", () => {
  it("端口 0 不裸显：输入框为空并出现随机端口语义提示", () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness values={values()} formRef={formRef}>
        <NetworkCard />
      </Harness>,
    );
    const quic = screen.getByLabelText("QUIC 端口") as HTMLInputElement;
    const tcp = screen.getByLabelText("TCP 端口") as HTMLInputElement;
    expect(quic.value).toBe("");
    expect(tcp.value).toBe("");
    expect(quic).toHaveAttribute("placeholder", "0 = 随机端口");
    expect(tcp).toHaveAttribute("placeholder", "0 = 随机端口");
    expect(
      screen.getAllByText("留空或填 0，节点启动时自动分配可用端口"),
    ).toHaveLength(2);
  });

  it("节点运行中就近展示当前实际生效端口；未运行不展示", () => {
    const formRef: FormRef = { current: null };
    const view = render(
      <Harness values={values()} formRef={formRef}>
        <NetworkCard />
      </Harness>,
    );
    expect(screen.queryByText(/当前实际生效端口/)).toBeNull();

    useNodeStore.setState(runningStatus(["0.0.0.0/u34000", "0.0.0.0/t34001"]));
    view.rerender(
      <Harness values={values()} formRef={formRef}>
        <NetworkCard />
      </Harness>,
    );
    expect(screen.getByText("当前实际生效端口：34000")).toBeInTheDocument();
    expect(screen.getByText("当前实际生效端口：34001")).toBeInTheDocument();
  });

  it("空地址列表显示出厂默认提示与恢复入口，点击写入表单并置脏", async () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness values={values()} formRef={formRef}>
        <AdvertiseCard />
      </Harness>,
    );
    expect(screen.getByText(/出厂默认观测端点/)).toHaveTextContent(
      "121.196.193.177:3402",
    );
    expect(screen.queryByText(/出厂默认引导端点/)).toBeNull();
    expect(screen.queryByText(/出厂默认中继端点/)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "恢复出厂默认" }));
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    expect(formRef.current?.getValues("observationAddrs")).toEqual([
      { value: "121.196.193.177:3402" },
    ]);
    expect(screen.queryByText(/出厂默认观测端点/)).toBeNull();
  });

  it("恢复的观测端点为 socket 语法且通过表单校验", async () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness values={values()} formRef={formRef}>
        <AdvertiseCard />
      </Harness>,
    );
    fireEvent.click(screen.getByRole("button", { name: "恢复出厂默认" }));
    await waitFor(() =>
      expect(formRef.current?.getValues("observationAddrs")).toEqual([
        { value: "121.196.193.177:3402" },
      ]),
    );
    let valid = false;
    await act(async () => {
      valid = (await formRef.current?.trigger("observationAddrs")) === true;
    });
    expect(valid).toBe(true);
    expect(screen.queryByText(/地址格式应为/)).toBeNull();
  });

  it("观测端口未设置时展示可选占位符", () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness values={values()} formRef={formRef}>
        <AdvertiseCard />
      </Harness>,
    );
    expect(screen.getByLabelText("观测端口")).toHaveAttribute(
      "placeholder",
      "可选，未设置",
    );
  });
});
