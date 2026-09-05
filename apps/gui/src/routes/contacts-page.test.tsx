import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import { ContactsPage } from "./contacts-page";

describe("ContactsPage 占位页", () => {
  it("三个区锚点目标存在，供命令面板 /contacts#* 深链定位", () => {
    render(<ContactsPage />);
    for (const id of ["friends", "groups", "agents"]) {
      expect(document.getElementById(id), "缺锚点: " + id).not.toBeNull();
    }
    expect(screen.getByRole("heading", { name: "通讯录", level: 1 })).toBeTruthy();
  });
});
