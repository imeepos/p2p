import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useEffect } from "react";
import { MemoryRouter } from "react-router-dom";
import {
  FormProvider,
  useForm,
  useFormContext,
  type UseFormReturn,
} from "react-hook-form";
import { describe, expect, it, vi } from "vitest";

const { ftpConfigSaveMock } = vi.hoisted(() => ({
  ftpConfigSaveMock: vi.fn(async () => true),
}));

// 集成段 mock：SettingsView 挂载即双读（config+ftp）并挂静态对端卡。
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
import {
  EMPTY_SETTINGS,
  settingsResolver,
  toFormValues,
  toFtpSaveInput,
  type SettingsFormValues,
} from "./config-schema";
import { FtpCard } from "./ftp-card";

type FormRef = { current: UseFormReturn<SettingsFormValues> | null };

const FTP_VIEW = { root: "/srv/ftp", authz: true, users: ["alice", "bob"] };

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
      <FtpCard />
    </Harness>,
  );
  let valid = false;
  await act(async () => {
    valid = (await formRef.current?.trigger(field)) === true;
  });
  return valid;
}

describe("FTP 表单值与保存载荷（W2b 空密码语义）", () => {
  it("toFormValues：既有用户行 existing=true 且密码一律留空（不回显）", () => {
    const values = toFormValues(
      {
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
      FTP_VIEW,
    );
    expect(values.ftpRoot).toBe("/srv/ftp");
    expect(values.ftpAuthz).toBe(true);
    expect(values.ftpAccounts).toEqual([
      { user: "alice", password: "", existing: true },
      { user: "bob", password: "", existing: true },
    ]);
  });

  it("toFormValues 缺省 ftp 视图 = 未装配（root 空表单挡保存）", () => {
    const values = toFormValues({
      quicPort: 0,
      tcpPort: 0,
      enableMdns: true,
      dataDir: "",
      bootstrap: [],
      relayAddrs: [],
      advertisedAddrs: [],
      observationPort: null,
      observationAddrs: [],
    });
    expect(values.ftpRoot).toBe("");
    expect(values.ftpAuthz).toBe(false);
    expect(values.ftpAccounts).toEqual([]);
  });

  it("toFtpSaveInput：既有用户密码留空发空串（=保留），新输入原样携带", () => {
    const values = toFormValues(
      {
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
      FTP_VIEW,
    );
    values.ftpAccounts = [
      { user: "alice", password: "", existing: true },
      { user: "bob", password: "new-secret", existing: true },
      { user: "carol", password: "carol-pass", existing: false },
    ];
    expect(toFtpSaveInput(values)).toEqual({
      root: "/srv/ftp",
      authz: true,
      accounts: {
        alice: "",
        bob: "new-secret",
        carol: "carol-pass",
      },
    });
  });
});

describe("FTP 字段校验矩阵", () => {
  const rootCases: Array<[Partial<SettingsFormValues>, boolean]> = [
    [{ ftpRoot: "/srv/ftp" }, true],
    // 未装配（未开鉴权且无账号）：root 空合法，可原样往返不挡保存
    [{ ftpRoot: "" }, true],
    // 开启鉴权即装配：root 必填
    [{ ftpRoot: "", ftpAuthz: true }, false],
    // 有账号即装配：root 必填
    [
      { ftpRoot: "", ftpAccounts: [{ user: "alice", password: "", existing: true }] },
      false,
    ],
  ];
  it.each(rootCases)("ftpRoot 场景 %j 校验通过性 %p", async (overrides, expected) => {
    expect(await isFieldValid(overrides, "ftpRoot")).toBe(expected);
  });

  const accountCases: Array<[SettingsFormValues["ftpAccounts"], boolean]> = [
    [[{ user: "alice", password: "", existing: true }], true],
    // 钉死语义：既有行空密码合法（保存发空串=保留原密码）
    [[{ user: "alice", password: "", existing: false }], false],
    [[{ user: "", password: "x", existing: false }], false],
    [
      [
        { user: "alice", password: "a", existing: true },
        { user: "alice", password: "", existing: true },
      ],
      false,
    ],
  ];
  it.each(accountCases)("ftpAccounts=%p 校验通过性 %p", async (rows, expected) => {
    expect(await isFieldValid({ ftpAccounts: rows }, "ftpAccounts")).toBe(expected);
  });
});

describe("FtpCard 控件", () => {
  it("根目录输入与鉴权开关写入表单并置脏", async () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness
        values={{
          ...EMPTY_SETTINGS,
          ftpRoot: "/srv/ftp",
          ftpAccounts: [{ user: "alice", password: "", existing: true }],
        }}
        formRef={formRef}
      >
        <FtpCard />
      </Harness>,
    );
    fireEvent.change(screen.getByLabelText("根目录"), {
      target: { value: "/srv/ftp2" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("form-dirty")).toHaveTextContent("dirty"),
    );
    expect(formRef.current?.getValues("ftpRoot")).toBe("/srv/ftp2");
    fireEvent.click(screen.getByLabelText("账号鉴权"));
    await waitFor(() => expect(formRef.current?.getValues("ftpAuthz")).toBe(true));
  });

  it("账号表可增删行；密码控件为 password 型且新增行必须显式密码", async () => {
    const formRef: FormRef = { current: null };
    render(
      <Harness
        values={{
          ...EMPTY_SETTINGS,
          ftpRoot: "/srv/ftp",
          ftpAccounts: [{ user: "alice", password: "", existing: true }],
        }}
        formRef={formRef}
      >
        <FtpCard />
      </Harness>,
    );
    const password = screen.getByLabelText("账号 1 密码") as HTMLInputElement;
    expect(password.type).toBe("password");
    expect(password.value).toBe("");
    fireEvent.click(screen.getByRole("button", { name: "添加账号" }));
    const newUser = screen.getByLabelText("账号 2 用户名") as HTMLInputElement;
    const newPassword = screen.getByLabelText("账号 2 密码") as HTMLInputElement;
    expect(newUser.value).toBe("");
    expect(newPassword.value).toBe("");
    fireEvent.change(newUser, { target: { value: "bob" } });
    fireEvent.change(newPassword, { target: { value: "bob-pass" } });
    await waitFor(() =>
      expect(formRef.current?.getValues("ftpAccounts")).toHaveLength(2),
    );
    // 删除账号 1 行后仅剩新增行
    fireEvent.click(screen.getByRole("button", { name: "删除账号 账号 1" }));
    await waitFor(() => {
      const rows = formRef.current?.getValues("ftpAccounts") ?? [];
      expect(rows).toHaveLength(1);
      expect(rows[0].user).toBe("bob");
    });
  });
});

describe("设置页集成：FTP 卡走既有保存/放弃流程", () => {
  it("编辑根目录后保存：configSave 与 ftpConfigSave 双写，脏状态归零", async () => {
    HTMLElement.prototype.scrollIntoView = vi.fn();
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <ThemeProvider>
          <ConfirmProvider>
            <SettingsView />
          </ConfirmProvider>
        </ThemeProvider>
      </MemoryRouter>,
    );
    // 等双读完成回显（根目录输入出现且值已载入）
    await waitFor(() => {
      const el = document.getElementById(
        "settings-ftp-root",
      ) as HTMLInputElement | null;
      if (el === null || el.value !== "/srv/ftp") {
        throw new Error("ftp config not loaded yet");
      }
    });
    const root = document.getElementById(
      "settings-ftp-root",
    ) as HTMLInputElement;
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
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <ThemeProvider>
          <ConfirmProvider>
            <SettingsView />
          </ConfirmProvider>
        </ThemeProvider>
      </MemoryRouter>,
    );
    await waitFor(() => {
      const el = document.getElementById(
        "settings-ftp-root",
      ) as HTMLInputElement | null;
      if (el === null || el.value !== "/srv/ftp") {
        throw new Error("ftp config not loaded yet");
      }
    });
    const root = document.getElementById(
      "settings-ftp-root",
    ) as HTMLInputElement;
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
