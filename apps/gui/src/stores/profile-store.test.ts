import { beforeEach, describe, expect, it, vi } from "vitest";

import type { NodeProfile } from "@/lib/ipc-types";

const { profileGetMock, profileSaveMock } = vi.hoisted(() => ({
  profileGetMock: vi.fn<() => Promise<NodeProfile>>(),
  profileSaveMock: vi.fn<(p: NodeProfile) => Promise<NodeProfile>>(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { profileGet: profileGetMock, profileSave: profileSaveMock },
}));

import { useProfileStore } from "./profile-store";

const saved: NodeProfile = { name: "家用节点", description: "客厅常开", avatar: null };

function resetStore(): void {
  useProfileStore.setState({
    profile: { name: "", description: "", avatar: null },
    loaded: false,
    loadError: null,
  });
}

beforeEach(() => {
  profileGetMock.mockReset();
  profileSaveMock.mockReset();
  resetStore();
});

describe("profile-store", () => {
  it("load 成功：写入 profile 并置 loaded", async () => {
    profileGetMock.mockResolvedValue(saved);
    await useProfileStore.getState().load();
    expect(useProfileStore.getState().profile).toEqual(saved);
    expect(useProfileStore.getState().loaded).toBe(true);
    expect(useProfileStore.getState().loadError).toBeNull();
  });

  it("load 失败：留 loadError 信号并原样上抛", async () => {
    profileGetMock.mockRejectedValue(new Error("读取失败"));
    await expect(useProfileStore.getState().load()).rejects.toThrow("读取失败");
    expect(useProfileStore.getState().loadError).toBe("读取失败");
    expect(useProfileStore.getState().loaded).toBe(false);
  });

  it("save 成功：以返回值更新 profile", async () => {
    profileSaveMock.mockResolvedValue(saved);
    await useProfileStore.getState().save(saved);
    expect(profileSaveMock).toHaveBeenCalledWith(saved);
    expect(useProfileStore.getState().profile).toEqual(saved);
    expect(useProfileStore.getState().loaded).toBe(true);
  });

  it("save 失败：不上抛吞没，profile 保持原值", async () => {
    profileSaveMock.mockRejectedValue(new Error("节点名称过长"));
    await expect(useProfileStore.getState().save(saved)).rejects.toThrow("节点名称过长");
    expect(useProfileStore.getState().profile).toEqual({ name: "", description: "", avatar: null });
  });
});
