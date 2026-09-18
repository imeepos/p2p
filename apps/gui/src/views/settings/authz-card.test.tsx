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
import { useAuthzStore } from "@/stores/authz-store";
import type { AuthzRoleView } from "@/lib/ipc-types";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  type SettingsFormValues,
} from "./config-schema";
import { AuthzCard } from "./authz-card";

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

function DirtyProbe() {
  const { formState } = useFormContext<SettingsFormValues>();
  return (
    <span data-testid="form-dirty">
      {formState.isDirty ? "dirty" : "clean"}
    </span>
  );
}

function Harness({
  formRef,
  children,
}: {
  formRef: FormRef;
  children: React.ReactNode;
}) {
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: EMPTY_SETTINGS,
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

function renderCard(): FormRef {
  const formRef: FormRef = { current: null };
  render(
    <Harness formRef={formRef}>
      <AuthzCard />
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

// 2026-09-18 IA 重组：默认角色行自 bootstrap-relay-card 迁出为独立权限卡，
// 用例随行迁移（选择器与 testid 不变，语义仍走整包保存）。
describe("AuthzCard 好友默认角色", () => {
  it("权限卡渲染角色选择器并读入当前值", () => {
    renderCard();
    expect(screen.getByText("权限")).toBeInTheDocument();
    expect(screen.getByText("加好友默认角色")).toBeInTheDocument();
    // EMPTY_SETTINGS 的 authzDefaultRole 出厂值为 friend
    expect(screen.getByTestId("settings-default-role").textContent).toBe(
      "好友",
    );
  });

  it("可选内建角色，也可选空串（禁用自动绑），选择置脏", async () => {
    const formRef = renderCard();
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
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
  });

  it("校验通过：触发整包保存路径不因角色行报错", async () => {
    const formRef = renderCard();
    let valid = false;
    await act(async () => {
      valid = (await formRef.current?.trigger()) === true;
    });
    expect(valid).toBe(true);
  });
});
