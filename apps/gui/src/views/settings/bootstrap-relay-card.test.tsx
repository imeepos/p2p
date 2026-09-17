import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import {
  FormProvider,
  useForm,
  useFormContext,
  type UseFormReturn,
} from "react-hook-form";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useAuthzStore } from "@/stores/authz-store";
import type { AuthzRoleView } from "@/lib/ipc-types";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  toGuiConfig,
  type SettingsFormValues,
} from "./config-schema";
import { BootstrapRelayCard } from "./bootstrap-relay-card";

const ROLES: AuthzRoleView[] = [
  { roleId: "friend", name: "好友", permissions: [], builtin: true, note: "" },
  {
    roleId: "operator",
    name: "操作员",
    permissions: [],
    builtin: true,
    note: "",
  },
];

type FormRef = { current: UseFormReturn<SettingsFormValues> | null };

const SEEDED: SettingsFormValues = {
  ...EMPTY_SETTINGS,
  bootstrap: [{ value: "192.168.1.10/u3400" }],
  relayAddrs: [{ value: "192.168.1.11/u3403" }],
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
    <ConfirmProvider>
      <FormProvider {...form}>
        {children}
        <DirtyProbe />
      </FormProvider>
    </ConfirmProvider>
  );
}

function renderCard(values: SettingsFormValues): FormRef {
  const formRef: FormRef = { current: null };
  render(
    <Harness values={values} formRef={formRef}>
      <BootstrapRelayCard />
    </Harness>,
  );
  return formRef;
}

beforeEach(() => {
  useAuthzStore.setState({ roles: ROLES, loadError: null });
});

// Unmount before resetting the store: React flushes pending passive effects
// on unmount, and a roles mutation while still mounted would re-run the
// card's load-once effect against the emptied store (noise, not a leak).
afterEach(() => {
  cleanup();
  useAuthzStore.setState({ roles: [], loadError: null });
});

describe("BootstrapRelayCard 三控件渲染矩阵", () => {
  it("三个编辑入口同卡渲染且读入当前值", () => {
    renderCard(SEEDED);
    expect(screen.getByText("rendezvous 地址簿")).toBeInTheDocument();
    expect(screen.getByText("中继地址列表")).toBeInTheDocument();
    expect(screen.getByText("加好友默认角色")).toBeInTheDocument();
    expect(
      (document.getElementById("bootstrap-row-0") as HTMLInputElement).value,
    ).toBe("192.168.1.10/u3400");
    expect(
      (document.getElementById("relayAddrs-row-0") as HTMLInputElement).value,
    ).toBe("192.168.1.11/u3403");
    expect(screen.getByTestId("settings-default-role").textContent).toBe(
      "好友",
    );
  });

  it("bootstrap 添加行写入表单并置脏", async () => {
    const formRef = renderCard(EMPTY_SETTINGS);
    fireEvent.click(screen.getAllByRole("button", { name: "添加地址" })[0]);
    fireEvent.change(document.getElementById("bootstrap-row-0")!, {
      target: { value: "43.240.223.138/u3400" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    expect(formRef.current?.getValues("bootstrap")).toEqual([
      { value: "43.240.223.138/u3400" },
    ]);
  });

  it("bootstrap 删除需二次确认：取消保留行，确认移除并置脏", async () => {
    const formRef = renderCard(SEEDED);
    fireEvent.click(screen.getAllByRole("button", { name: "删除地址" })[0]);
    expect(
      await screen.findByText("将从地址簿中移除 192.168.1.10/u3400，删除后需重启节点生效。"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() =>
      expect(screen.queryByText("删除引导地址？")).not.toBeInTheDocument(),
    );
    expect(formRef.current?.getValues("bootstrap")).toHaveLength(1);

    fireEvent.click(screen.getAllByRole("button", { name: "删除地址" })[0]);
    await screen.findByText("删除引导地址？");
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    await waitFor(() =>
      expect(formRef.current?.getValues("bootstrap")).toEqual([]),
    );
    expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty");
  });

  it("relay 行编辑写入表单并置脏", async () => {
    const formRef = renderCard(EMPTY_SETTINGS);
    fireEvent.click(screen.getAllByRole("button", { name: "添加地址" })[1]);
    fireEvent.change(document.getElementById("relayAddrs-row-0")!, {
      target: { value: "121.196.193.177/u3403" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    expect(formRef.current?.getValues("relayAddrs")).toEqual([
      { value: "121.196.193.177/u3403" },
    ]);
  });

  it("默认角色可选内建角色，也可选空串（禁用自动绑）", async () => {
    const formRef = renderCard(EMPTY_SETTINGS);
    fireEvent.click(screen.getByTestId("settings-default-role"));
    fireEvent.click(screen.getByRole("option", { name: "操作员" }));
    await waitFor(() =>
      expect(formRef.current?.getValues("authzDefaultRole")).toBe("operator"),
    );
    fireEvent.click(screen.getByTestId("settings-default-role"));
    fireEvent.click(screen.getByRole("option", { name: "不自动绑定" }));
    await waitFor(() =>
      expect(formRef.current?.getValues("authzDefaultRole")).toBe(""),
    );
    expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty");
  });

  it("保存路径：trigger 通过后 toGuiConfig 携带三字段（configSave 整包防丢）", async () => {
    const formRef = renderCard(SEEDED);
    let valid = false;
    await act(async () => {
      valid = (await formRef.current?.trigger()) === true;
    });
    expect(valid).toBe(true);
    const saved = toGuiConfig(formRef.current!.getValues());
    expect(saved.bootstrap).toEqual(["192.168.1.10/u3400"]);
    expect(saved.relayAddrs).toEqual(["192.168.1.11/u3403"]);
    expect(saved.authzDefaultRole).toBe("friend");
  });

  it("放弃路径：reset 还原种子值且脏标记归零", async () => {
    console.log("PROBE beforeEach-after store roles:", useAuthzStore.getState().roles.length);
    const formRef = renderCard(SEEDED);
    console.log("PROBE post-render store roles:", useAuthzStore.getState().roles.length);
    fireEvent.click(screen.getAllByRole("button", { name: "添加地址" })[0]);
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    act(() => {
      formRef.current?.reset(SEEDED);
    });
    expect(screen.getByTestId("form-dirty")).toHaveTextContent("clean");
    expect(formRef.current?.getValues("bootstrap")).toEqual(
      SEEDED.bootstrap,
    );
  });
});
