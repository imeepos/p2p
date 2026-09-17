import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// 类型别名仅类型空间使用（vi.hoisted 先于 import 求值，运行时无引用）。
type MockPeer = { peerId: string; addrs: string[]; note: string };

const { listMock, upsertMock, removeMock } = vi.hoisted(() => ({
  listMock: vi.fn(async (): Promise<{ peers: MockPeer[] }> => ({ peers: [] })),
  upsertMock: vi.fn(async (): Promise<boolean> => true),
  removeMock: vi.fn(async (): Promise<boolean> => true),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    staticPeersList: (...args: unknown[]) => listMock(...(args as [])),
    staticPeersUpsert: (...args: unknown[]) => upsertMock(...(args as [])),
    staticPeersRemove: (...args: unknown[]) => removeMock(...(args as [])),
  },
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { StaticPeersCard } from "./static-peers-card";

const PEER_A = "a".repeat(44);
const PEER_B = "b".repeat(44);

function renderCard() {
  return render(
    <ConfirmProvider>
      <StaticPeersCard />
    </ConfirmProvider>,
  );
}

// 模块级 mock 跨用例残留（mockResolvedValueOnce 未消费即泄漏到下一用例），
// 每条用例前统一重置回默认实现。
beforeEach(() => {
  listMock.mockReset().mockResolvedValue({ peers: [] });
  upsertMock.mockReset().mockResolvedValue(true);
  removeMock.mockReset().mockResolvedValue(true);
});

async function openAddEditor() {
  renderCard();
  fireEvent.click(await screen.findByTestId("static-peers-add"));
  await screen.findByTestId("static-peers-editor");
}

function fillPeerId(value: string) {
  fireEvent.change(screen.getByLabelText("PeerId"), {
    target: { value },
  });
  fireEvent.blur(screen.getByLabelText("PeerId"));
}

describe("静态对端卡渲染矩阵", () => {
  it("空簿展示空态与添加入口", async () => {
    renderCard();
    expect(await screen.findByTestId("static-peers-empty")).toBeInTheDocument();
    expect(screen.getByTestId("static-peers-add")).toBeInTheDocument();
  });

  it("新增：PeerId 走 base58 预检，失焦即拒绝非法值", async () => {
    await openAddEditor();
    fillPeerId("not-base58!");
    await screen.findByText("PeerId 应为 43-45 位 base58 字符");
    // 修正为合法后错误消失
    fillPeerId(PEER_A);
    await waitFor(() =>
      expect(screen.queryByText("PeerId 应为 43-45 位 base58 字符")).toBeNull(),
    );
  });

  it("新增：非法地址被提交校验挡下，upsert 不落盘", async () => {
    await openAddEditor();
    fillPeerId(PEER_A);
    fireEvent.click(screen.getByRole("button", { name: "添加地址" }));
    fireEvent.change(screen.getByLabelText("地址 1"), {
      target: { value: "10.0.0.1:4222" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存对端" }));
    // 地址行校验在提交时把关：非法语法红字可见且 upsert 不被调用
    await screen.findByText("地址格式应为 ip/u端口（QUIC）或 ip/t端口（TCP）");
    expect(upsertMock).not.toHaveBeenCalled();
  });

  it("新增：合法条目 upsert 落盘并刷新列表，编辑器关闭", async () => {
    await openAddEditor();
    fillPeerId(PEER_A);
    fireEvent.click(screen.getByRole("button", { name: "添加地址" }));
    fireEvent.change(screen.getByLabelText("地址 1"), {
      target: { value: "10.0.0.1/u4222" },
    });
    fireEvent.change(screen.getByLabelText("备注"), {
      target: { value: "edge-1" },
    });
    // 先桩后动作：保存成功后的列表回读返回已存条目
    listMock.mockResolvedValueOnce({
      peers: [{ peerId: PEER_A, addrs: ["10.0.0.1/u4222"], note: "edge-1" }],
    });
    fireEvent.click(screen.getByRole("button", { name: "保存对端" }));
    await waitFor(() => {
      expect(upsertMock).toHaveBeenCalledWith(
        PEER_A,
        ["10.0.0.1/u4222"],
        "edge-1",
      );
    });
    await screen.findByTestId(`static-peer-row-${PEER_A}`);
    expect(screen.queryByTestId("static-peers-editor")).toBeNull();
  });

  it("编辑：PeerId 锁定不可改，地址与备注可更新", async () => {
    listMock.mockResolvedValue({
      peers: [{ peerId: PEER_B, addrs: ["10.0.0.2/t4000"], note: "old" }],
    });
    renderCard();
    fireEvent.click(await screen.findByTestId(`static-peer-edit-${PEER_B}`));
    await screen.findByTestId("static-peers-editor");
    const peerInput = screen.getByLabelText("PeerId") as HTMLInputElement;
    expect(peerInput.disabled).toBe(true);
    expect(peerInput.value).toBe(PEER_B);
    const addrInput = screen.getByLabelText("地址 1");
    fireEvent.change(addrInput, { target: { value: "10.0.0.3/u5000" } });
    fireEvent.change(screen.getByLabelText("备注"), {
      target: { value: "new" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存对端" }));
    await waitFor(() => {
      expect(upsertMock).toHaveBeenCalledWith(
        PEER_B,
        ["10.0.0.3/u5000"],
        "new",
      );
    });
  });

  it("upsert 失败：编辑器驻留不吞错，可重试", async () => {
    upsertMock.mockRejectedValueOnce(new Error("落盘失败（mock）"));
    await openAddEditor();
    fillPeerId(PEER_A);
    fireEvent.click(screen.getByRole("button", { name: "添加地址" }));
    fireEvent.change(screen.getByLabelText("地址 1"), {
      target: { value: "10.0.0.1/u4222" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "保存对端" }));
    });
    await waitFor(() => expect(upsertMock).toHaveBeenCalled());
    // 编辑器保持打开：失败可观测可重试
    expect(screen.getByTestId("static-peers-editor")).toBeInTheDocument();
  });

  it("删除：确认弹框取消不删；确认后 remove 并刷新", async () => {
    listMock.mockResolvedValue({
      peers: [{ peerId: PEER_A, addrs: ["10.0.0.1/u4222"], note: "" }],
    });
    renderCard();
    fireEvent.click(await screen.findByTestId(`static-peer-remove-${PEER_A}`));
    // 二次确认：先取消
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    expect(removeMock).not.toHaveBeenCalled();
    // 再删：先桩后动作（删除成功后的回读返回空簿），确认删除
    fireEvent.click(await screen.findByTestId(`static-peer-remove-${PEER_A}`));
    listMock.mockResolvedValueOnce({ peers: [] });
    fireEvent.click(await screen.findByRole("button", { name: "删除" }));
    await waitFor(() => {
      expect(removeMock).toHaveBeenCalledWith(PEER_A);
    });
    await screen.findByTestId("static-peers-empty");
  });
});
