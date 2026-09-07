import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

const JSON_TEXT = /"type": "dial_failed"/;

async function expandDetail() {
  render(<Harness payload={event} />);
  fireEvent.click(screen.getByTestId("event-row-details"));
  expect(screen.getByTestId("event-row-details")).toHaveAttribute(
    "aria-expanded",
    "true",
  );
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

  it("行尾「详情」按钮展开：data-state=open + aria-expanded", () => {
    render(<Harness payload={event} />);
    const details = screen.getByTestId("event-row-details");
    expect(details).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(details);
    const row = screen.getByTestId("event-row-details").closest("[data-state]");
    expect(row).toHaveAttribute("data-state", "open");
    expect(details).toHaveAttribute("aria-expanded", "true");
  });
});

// R2-16 回归：原始负载 JSON 默认折叠，提供展开切换与「复制详情」。
describe("EventRowDetail 负载折叠与复制（R2-16）", () => {
  it("详情展开后原始负载默认折叠，不直排 JSON 原文", async () => {
    await expandDetail();
    expect(screen.getByText("原始负载")).toBeInTheDocument();
    expect(screen.queryByText(JSON_TEXT)).toBeNull();
  });

  it("「展开 JSON/收起 JSON」切换可见性与 aria-expanded", async () => {
    await expandDetail();
    const toggle = screen.getByTestId("event-detail-json-toggle");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText(JSON_TEXT)).toBeNull();
    fireEvent.click(toggle);
    expect(screen.getByText(JSON_TEXT)).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("收起 JSON")).toBeInTheDocument();
    fireEvent.click(toggle);
    expect(screen.queryByText(JSON_TEXT)).toBeNull();
  });

  it("「复制详情」写入完整 JSON 负载（折叠态同样可复制）", async () => {
    await expandDetail();
    const writeText = vi.fn(async () => undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    fireEvent.click(screen.getByTestId("event-detail-copy"));
    await waitFor(() => expect(writeText).toHaveBeenCalled());
    const written = String(writeText.mock.calls[0]?.[0]);
    expect(JSON.parse(written)).toMatchObject({
      type: "dial_failed",
      reason: "timeout",
    });
  });

  it("收起再展开状态一致：负载内容不变", async () => {
    await expandDetail();
    fireEvent.click(screen.getByTestId("event-detail-json-toggle"));
    const payloadOf = () => screen.getByText(JSON_TEXT).textContent;
    const first = payloadOf();
    fireEvent.click(screen.getByTestId("event-row-details"));
    expect(screen.queryByText(JSON_TEXT)).toBeNull();
    fireEvent.click(screen.getByTestId("event-row-details"));
    fireEvent.click(screen.getByTestId("event-detail-json-toggle"));
    expect(payloadOf()).toBe(first);
  });
});
