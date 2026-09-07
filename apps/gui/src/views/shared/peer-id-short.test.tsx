import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import { shortPeerId } from "@/lib/peer-name";
import { PeerIdShort } from "./peer-id-short";

const FULL = "vKLTAv6c8dEfGhIjKlMnOpQrStUvWxyzAbCdEfGhIjkl";

// R2-17 回归：全站 PeerId 缩略唯一口径（前 6…后 4 + 悬停完整）。
describe("PeerIdShort 缩略口径", () => {
  it("长 PeerId 渲染前 6…后 4，title 悬停完整 ID", () => {
    render(<PeerIdShort peerId={FULL} />);
    const span = screen.getByTitle(FULL);
    expect(span).toBeInTheDocument();
    expect(span).toHaveTextContent(shortPeerId(FULL));
    expect(span).toHaveTextContent("…" + FULL.slice(-4));
    expect(span.textContent).not.toContain(FULL);
  });

  it("短于缩略窗口的 ID 原样展示，不省略", () => {
    const shortId = "abc123";
    render(<PeerIdShort peerId={shortId} />);
    expect(screen.getByTitle(shortId)).toHaveTextContent(shortId);
  });
});
