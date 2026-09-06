import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { describe, expect, it } from "vitest";

import "@/i18n";
import { LlmShareEntryCard } from "./llm-share-entry-card";

function LocationProbe() {
  const location = useLocation();
  return <span data-testid="loc">{location.pathname}</span>;
}

describe("settings llm-share 入口卡（契约 v11 §16.3）", () => {
  it("一句话说明含默认拒绝心智，点击打开跳转 /llm-share", () => {
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <LlmShareEntryCard />
        <Routes>
          <Route path="*" element={<LocationProbe />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(screen.getByText("LLM 共享")).toBeInTheDocument();
    expect(screen.getByText(/allowlist 无条目即不可用/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "打开" }));
    expect(screen.getByTestId("loc")).toHaveTextContent("/llm-share");
  });
});
