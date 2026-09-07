import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { RecentEventLine } from "./recent-event-line";

const PEER_ID = "a".repeat(44);

const EVENT: NodeEventJson = {
  type: "peer_discovered",
  peer: PEER_ID,
  addrs: ["192.168.1.5/u40001"],
  source: "mdns",
  tsMs: Date.now(),
};

describe("RecentEventLine tooltip 本地化", () => {
  it("tooltip 用 i18n 摘要且带完整 PeerId，不再是原始协议串", () => {
    render(<RecentEventLine event={EVENT} locale="zh-CN" now={Date.now()} />);
    const span = screen.getByTitle(
      "发现节点 " + PEER_ID + "（192.168.1.5/u40001）",
    );
    expect(span).toBeInTheDocument();
    expect(document.querySelector('[title^="peer_discovered"]')).toBeNull();
  });

  // R2-17 回归：概览最近事件与事件页同一缩略口径（前 6…后 4），
  // 不再出现 8 位前缀第三种截断。
  it("行内摘要 PeerId 缩略与事件页同口径（前 6…后 4）", () => {
    render(<RecentEventLine event={EVENT} locale="zh-CN" now={Date.now()} />);
    expect(
      screen.getByText(
        "发现节点 aaaaaa…" + PEER_ID.slice(-4) + "（192.168.1.5/u40001）",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("发现节点 " + PEER_ID.slice(0, 8))).toBeNull();
  });
});
