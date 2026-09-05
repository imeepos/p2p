import { act, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import { MENU_ENTRIES } from "@/config/menu.def";
import { useNumberRouteHotkeys } from "./use-hotkeys";

function LocationProbe() {
  useNumberRouteHotkeys();
  const location = useLocation();
  return <div data-testid="loc">path:{location.pathname}</div>;
}

function renderProbe(initial = "/") {
  return render(
    <MemoryRouter initialEntries={[initial]}>
      <Routes>
        <Route path="*" element={<LocationProbe />} />
      </Routes>
    </MemoryRouter>,
  );
}

function pressDigitOn(target: EventTarget, digit: string, modifier: boolean): void {
  act(() => {
    target.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: digit,
        metaKey: modifier,
        bubbles: true,
      }),
    );
  });
}

function currentPath(): string {
  return screen.getByTestId("loc").textContent ?? "";
}

function mountOpenDialog(): HTMLElement {
  const dialog = document.createElement("div");
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("data-state", "open");
  document.body.appendChild(dialog);
  return dialog;
}

afterEach(() => {
  document.querySelectorAll('[role="dialog"], [role="alertdialog"]').forEach((el) => el.remove());
});

describe("useNumberRouteHotkeys", () => {
  it("Cmd/Ctrl+1..9 跳转前九个注册路由，0 不跳页", () => {
    renderProbe();
    for (let digit = 1; digit <= 9; digit += 1) {
      pressDigitOn(window, String(digit), true);
      expect(currentPath()).toBe("path:" + MENU_ENTRIES[digit - 1].path);
    }
    pressDigitOn(window, "0", true);
    expect(currentPath()).toBe("path:" + MENU_ENTRIES[8].path);
  });

  it("无修饰键的数字不跳页", () => {
    renderProbe();
    pressDigitOn(window, "1", false);
    expect(currentPath()).toBe("path:/");
  });

  it("存在打开的对话框时数字热键不切页", () => {
    renderProbe();
    mountOpenDialog();
    pressDigitOn(window, "1", true);
    expect(currentPath()).toBe("path:/");
  });

  it("焦点在对话框内时数字热键不切页", () => {
    renderProbe();
    const dialog = mountOpenDialog();
    pressDigitOn(dialog, "1", true);
    expect(currentPath()).toBe("path:/");
  });
});
