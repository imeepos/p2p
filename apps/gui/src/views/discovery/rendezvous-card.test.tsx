import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import { RendezvousCard } from "./rendezvous-card";

function renderCard(bootstrap: string[]) {
  return render(
    <ConfirmProvider>
      <RendezvousCard
        bootstrap={bootstrap}
        onChange={async () => true}
        addOpen={false}
        onAddOpenChange={() => {}}
      />
    </ConfirmProvider>,
  );
}

describe("RendezvousCard 添加按钮唯一性（需求 6）", () => {
  it("地址簿为空时全卡仅一个「添加地址」按钮（空态内）", () => {
    renderCard([]);
    const buttons = screen.getAllByRole("button", { name: "添加地址" });
    expect(buttons).toHaveLength(1);
    expect(screen.getByText(/地址簿为空/)).toBeTruthy();
  });

  it("地址簿非空时同样仅一个「添加地址」按钮", () => {
    renderCard(["/ip4/203.0.113.5/udp/3400"]);
    const buttons = screen.getAllByRole("button", { name: "添加地址" });
    expect(buttons).toHaveLength(1);
    const table = screen.getByRole("table");
    expect(within(table).getAllByRole("row")).toHaveLength(2);
  });
});
