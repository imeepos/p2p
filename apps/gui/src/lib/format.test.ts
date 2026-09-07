import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { formatConversationTime } from "./format";

// W1-03 回归：聊天时间跨天语义。今天 HH:mm；昨天「昨天 HH:mm」；
// 更早带日期。固定系统时间锚点，避免测试随挂钟漂移。
const TODAY = new Date("2026-09-07T09:05:00").getTime();
const YESTERDAY = new Date("2026-09-06T23:30:00").getTime();
const LAST_WEEK = new Date("2026-09-01T08:15:00").getTime();

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-09-07T15:00:00"));
});

afterEach(() => {
  vi.useRealTimers();
});

describe("formatConversationTime 跨天时间语义（W1-03）", () => {
  it("今天只显 HH:mm", () => {
    const text = formatConversationTime(TODAY, "zh-CN");
    expect(text).toMatch(/09:05/);
    expect(text).not.toContain("9月");
    expect(text).not.toContain("昨天");
  });

  it("昨天补「昨天」前缀但保留时分", () => {
    const text = formatConversationTime(YESTERDAY, "zh-CN");
    expect(text).toContain("昨天");
    expect(text).toMatch(/23:30/);
  });

  it("更早显日期+时分，不再伪装成当天", () => {
    const text = formatConversationTime(LAST_WEEK, "zh-CN");
    expect(text).toContain("9月1日");
    expect(text).toMatch(/08:15/);
  });
});
