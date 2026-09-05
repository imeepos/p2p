import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import { DataLinkBanner } from "./data-link-banner";

function reset(
  over: Partial<{
    bootstrapPhase: "idle" | "loading" | "ready" | "error";
    bootstrapError: string | null;
    dataStale: boolean;
    lastRefreshError: string | null;
  }> = {},
): void {
  useNodeStore.setState({
    bootstrapPhase: "idle",
    bootstrapError: null,
    dataStale: false,
    lastRefreshError: null,
    ...over,
  });
}

describe("DataLinkBanner", () => {
  beforeEach(() => reset());

  it("引导失败给显式错误态与重试入口，而非永挂骨架", () => {
    reset({ bootstrapPhase: "error", bootstrapError: "sub boom" });
    render(<DataLinkBanner />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText("数据链路未就绪：事件订阅或初始刷新失败"),
    ).toBeInTheDocument();
    expect(screen.getByText("sub boom")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  });

  it("刷新连败显示数据可能已过期，恢复后横幅自动消失", () => {
    reset({ dataStale: true, lastRefreshError: "refresh boom" });
    const { container, rerender } = render(<DataLinkBanner />);
    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(
      screen.getByText("数据可能已过期：状态刷新连续失败"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "立即刷新" }),
    ).toBeInTheDocument();

    // store 恢复（refresh 成功置 dataStale=false）→ 横幅派生消失。
    reset();
    rerender(<DataLinkBanner />);
    expect(screen.queryByRole("status")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("正常态不渲染横幅，不把最后一次数据当实时数据打扰用户", () => {
    reset({ bootstrapPhase: "ready" });
    const { container } = render(<DataLinkBanner />);
    expect(container).toBeEmptyDOMElement();
  });
});
