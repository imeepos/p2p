import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { NodeProfile } from "@/lib/ipc-types";

const { profileGetMock, profileSaveMock } = vi.hoisted(() => ({
  profileGetMock: vi.fn<() => Promise<NodeProfile>>(),
  profileSaveMock: vi.fn<(p: NodeProfile) => Promise<NodeProfile>>(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { profileGet: profileGetMock, profileSave: profileSaveMock },
}));

import "@/i18n";
import { useProfileStore } from "@/stores/profile-store";
import { ProfileCard } from "./profile-card";

const saved: NodeProfile = {
  name: "家用节点",
  description: "客厅常开的中继兜底节点",
  avatar: null,
};

async function mountWith(profile: NodeProfile): Promise<void> {
  profileGetMock.mockResolvedValue(profile);
  render(<ProfileCard />);
  await waitFor(() => {
    expect(profileGetMock).toHaveBeenCalled();
  });
}

beforeEach(() => {
  profileGetMock.mockReset();
  profileSaveMock.mockReset();
  useProfileStore.setState({
    profile: { name: "", description: "", avatar: null },
    loaded: false,
    loadError: null,
  });
});

describe("ProfileCard", () => {
  it("加载后回显已保存资料，保存按钮初始禁用", async () => {
    await mountWith(saved);
    const name = screen.getByLabelText("节点名称") as HTMLInputElement;
    expect(name.value).toBe("家用节点");
    expect(screen.getByRole("button", { name: "保存资料" })).toBeDisabled();
  });

  it("编辑后保存：profileSave 收到 trim 后的值并回读刷新", async () => {
    await mountWith(saved);
    const name = screen.getByLabelText("节点名称");
    fireEvent.change(name, { target: { value: "  工作机  " } });
    const save = screen.getByRole("button", { name: "保存资料" });
    expect(save).toBeEnabled();
    profileSaveMock.mockResolvedValue({ ...saved, name: "工作机" });
    fireEvent.click(save);
    await waitFor(() => {
      expect(profileSaveMock).toHaveBeenCalledWith(
        expect.objectContaining({ name: "工作机", description: "客厅常开的中继兜底节点" }),
      );
    });
    await waitFor(() => {
      expect((screen.getByLabelText("节点名称") as HTMLInputElement).value).toBe("工作机");
    });
  });

  it("加载失败出重试入口", async () => {
    profileGetMock.mockRejectedValue(new Error("读取失败"));
    render(<ProfileCard />);
    await waitFor(() => {
      expect(screen.getByText("节点资料加载失败，请重试")).toBeTruthy();
    });
  });
});
