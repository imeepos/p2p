import { cleanup, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import { MENU_ENTRIES } from "@/config/menu.def";
import "@/i18n";
import { Sidebar } from "./sidebar";

function renderSidebar(collapsed: boolean) {
  return render(
    <MemoryRouter>
      <Sidebar collapsed={collapsed} onToggle={() => {}} />
    </MemoryRouter>,
  );
}

afterEach(() => cleanup());

describe("Sidebar 数字快捷键徽标", () => {
  it("未折叠时前九个导航项行尾显示对应数字徽标，第 10 项无徽标", () => {
    renderSidebar(false);
    const links = within(screen.getByRole("navigation")).getAllByRole("link");
    expect(links).toHaveLength(MENU_ENTRIES.length);
    links.forEach((link, index) => {
      const badge = link.querySelector("kbd");
      if (index < 9) {
        expect(badge?.textContent).toBe(String(index + 1));
      } else {
        expect(badge).toBeNull();
      }
    });
  });

  it("折叠时不显示数字徽标", () => {
    renderSidebar(true);
    const links = within(screen.getByRole("navigation")).getAllByRole("link");
    expect(links).toHaveLength(MENU_ENTRIES.length);
    links.forEach((link) => {
      expect(link.querySelector("kbd")).toBeNull();
    });
  });
});
