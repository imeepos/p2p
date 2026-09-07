import { describe, expect, it } from "vitest";

import { LlmShareMockError } from "./mock-borrow";
import { isOfferNotPublished } from "./offer-errors";

describe("offer show 未发布语义识别（R2-03）", () => {
  it("mock 未发布报错识别为空态语义", () => {
    expect(isOfferNotPublished(new LlmShareMockError("never published: run offer publish first"))).toBe(true);
  });

  it("中文变体与字符串形态同样识别", () => {
    expect(isOfferNotPublished(new Error("offer 从未发布"))).toBe(true);
    expect(isOfferNotPublished("尚未发布: run offer publish first")).toBe(true);
  });

  it("真实错误不误判", () => {
    expect(isOfferNotPublished(new Error("connection refused"))).toBe(false);
    expect(isOfferNotPublished(new LlmShareMockError("boom"))).toBe(false);
  });
});
