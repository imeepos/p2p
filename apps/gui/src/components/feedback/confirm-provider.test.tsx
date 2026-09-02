import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConfirmProvider, useConfirm } from "./confirm-provider";

function Host({ onResult }: { onResult: (value: boolean) => void }) {
  const confirm = useConfirm();
  return (
    <button
      type="button"
      onClick={() =>
        void confirm({
          title: "危险操作?",
          description: "不可撤销",
          confirmText: "确认删除",
          cancelText: "先不了",
          destructive: true,
        }).then(onResult)
      }
    >
      ask
    </button>
  );
}

describe("useConfirm", () => {
  it("确认路径返回 true 并关闭弹框", async () => {
    const onResult = vi.fn();
    render(
      <ConfirmProvider>
        <Host onResult={onResult} />
      </ConfirmProvider>,
    );
    fireEvent.click(screen.getByText("ask"));
    expect(await screen.findByText("危险操作?")).toBeInTheDocument();
    fireEvent.click(screen.getByText("确认删除"));
    await vi.waitFor(() => expect(onResult).toHaveBeenCalledWith(true));
    expect(screen.queryByText("危险操作?")).not.toBeInTheDocument();
  });

  it("取消路径返回 false 并关闭弹框", async () => {
    const onResult = vi.fn();
    render(
      <ConfirmProvider>
        <Host onResult={onResult} />
      </ConfirmProvider>,
    );
    fireEvent.click(screen.getByText("ask"));
    expect(await screen.findByText("危险操作?")).toBeInTheDocument();
    fireEvent.click(screen.getByText("先不了"));
    await vi.waitFor(() => expect(onResult).toHaveBeenCalledWith(false));
    expect(screen.queryByText("危险操作?")).not.toBeInTheDocument();
  });
});
