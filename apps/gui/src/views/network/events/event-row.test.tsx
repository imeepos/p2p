import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { EventRow } from "./event-row";

const event: NodeEventJson = {
  type: "dial_failed",
  peer: "peer-abcdef1234567890",
  reason: "timeout",
  tsMs: 1757200000000,
};

// 受控展开态的最小宿主：与 EventsView 的 Set 语义一致（点击切换）。
function Harness({ payload }: { payload: NodeEventJson }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <EventRow
      event={payload}
      locale="zh-CN"
      expanded={expanded}
      onToggle={() => setExpanded((v) => !v)}
    />
  );
}

function renderRow(payload: NodeEventJson = event) {
  const onToggle = vi.fn();
  const view = render(
    <EventRow event={payload} locale="zh-CN" expanded={false} onToggle={onToggle} />,
  );
  return { onToggle, view };
}

describe("EventRow 展开反馈（F16）", () => {
  it("收起态：data-state=closed，无详情负载", () => {
    renderRow();
    const row = screen.getByText("详情").closest("[data-state]");
    expect(row).toHaveAttribute("data-state", "closed");
    // 「原始负载」标签只在展开详情区渲染
    expect(screen.queryByText("原始负载")).toBeNull();
  });

  it("行主体点击回调 onToggle（展开由受控态驱动）", () => {
    const { onToggle } = renderRow();
    fireEvent.click(screen.getByRole("button", { name: /拨号失败/ }));
    expect(onToggle).toHaveBeenCalledWith(event);
  });

  it("行尾「详情」按钮展开：data-state=open + aria-expanded + 负载可见", () => {
    render(<Harness payload={event} />);
    const details = screen.getByTestId("event-row-details");
    expect(details).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(details);
    const row = screen.getByTestId("event-row-details").closest("[data-state]");
    expect(row).toHaveAttribute("data-state", "open");
    expect(details).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText(/"type": "dial_failed"/)).toBeInTheDocument();
  });

  it("收起再展开状态一致：负载内容不变", () => {
    render(<Harness payload={event} />);
    const details = screen.getByTestId("event-row-details");
    const payloadOf = () =>
      screen.getByText(/"type": "dial_failed"/).textContent;
    fireEvent.click(details);
    const first = payloadOf();
    fireEvent.click(details);
    expect(screen.queryByText(/"type": "dial_failed"/)).toBeNull();
    fireEvent.click(details);
    expect(payloadOf()).toBe(first);
  });
});
