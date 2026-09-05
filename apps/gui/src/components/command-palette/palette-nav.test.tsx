import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PALETTE_NAV_ENTRIES } from "@/config/palette-nav";
import i18n from "@/i18n";
import { CommandPalette } from "./command-palette";

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

function LocationProbe() {
  const location = useLocation();
  return (
    <div data-testid="loc">
      {location.pathname + location.search + location.hash}
    </div>
  );
}

function renderPaletteWithLocation(onOpenChange: (open: boolean) => void) {
  return render(
    <MemoryRouter initialEntries={["/start"]}>
      <LocationProbe />
      <CommandPalette open onOpenChange={onOpenChange} />
    </MemoryRouter>,
  );
}

afterEach(() => cleanup());

describe("面板导航注册表（5.2：13 项 = 4 rail + 6 网络 tab + 3 通讯录锚点）", () => {
  it("注册表恰好 13 项且构成符合拍板基线", () => {
    expect(PALETTE_NAV_ENTRIES).toHaveLength(13);
    expect(
      PALETTE_NAV_ENTRIES.filter((entry) => !entry.path.includes("/network/") && !entry.path.includes("#")),
    ).toHaveLength(4);
    expect(
      PALETTE_NAV_ENTRIES.filter((entry) => entry.path.startsWith("/network/")),
    ).toHaveLength(6);
    expect(
      PALETTE_NAV_ENTRIES.filter((entry) => entry.path.startsWith("/contacts#")),
    ).toHaveLength(3);
  });

  it("13 个导航项逐一可达且点击后关闭面板", async () => {
    for (const entry of PALETTE_NAV_ENTRIES) {
      const onOpenChange = vi.fn();
      renderPaletteWithLocation(onOpenChange);
      await screen.findByRole("dialog");
      const label = i18n.t(entry.labelKey);
      const item = screen
        .getAllByRole("option")
        .find((el) => el.textContent?.includes(label));
      expect(item, "缺少导航项: " + label).toBeTruthy();
      fireEvent.click(item as HTMLElement);
      expect(
        screen.getByTestId("loc").textContent,
        "导航未达: " + entry.path,
      ).toBe(entry.path);
      expect(onOpenChange).toHaveBeenCalledWith(false);
      cleanup();
    }
  });
});
