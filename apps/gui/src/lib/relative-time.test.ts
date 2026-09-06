import { describe, expect, it } from "vitest";

import { formatRelative, justNowLabel } from "./relative-time";

const NOW = 1_700_000_000_000;

describe("formatRelative F18 钳制", () => {
  it("未来时刻（时钟偏斜/事件乱序）落「刚刚」下限，不再出现未来时", () => {
    expect(formatRelative(NOW + 60_000, "zh-CN", NOW)).toBe("刚刚");
    expect(formatRelative(NOW + 2_000, "zh-CN", NOW)).toBe("刚刚");
    expect(formatRelative(NOW + 3_600_000, "en-US", NOW)).toBe("just now");
  });

  it("1 秒内与边界当前时刻同为「刚刚」", () => {
    expect(formatRelative(NOW - 200, "zh-CN", NOW)).toBe("刚刚");
    expect(formatRelative(NOW, "zh-CN", NOW)).toBe("刚刚");
  });

  it("过去时间按秒/分/时/日阶梯换算，符号恒为过去", () => {
    expect(formatRelative(NOW - 2_000, "zh-CN", NOW)).toMatch(/2秒钟前|2 秒前/);
    expect(formatRelative(NOW - 120_000, "zh-CN", NOW)).toMatch(/2分钟前|2 分钟前/);
    expect(formatRelative(NOW - 7_200_000, "en-US", NOW)).toContain("hour");
    expect(formatRelative(NOW - 172_800_000, "en-US", NOW)).toContain("day");
  });

  it("缺省 now 参数用当前时钟（真实调用路径）", () => {
    expect(formatRelative(Date.now() + 5_000, "zh-CN")).toBe("刚刚");
    expect(formatRelative(Date.now() - 60_000, "en-US")).toContain("minute");
  });
});

describe("justNowLabel", () => {
  it("已知语言取对应文案，未知语言回退英文", () => {
    expect(justNowLabel("zh-CN")).toBe("刚刚");
    expect(justNowLabel("en-US")).toBe("just now");
    expect(justNowLabel("xx-XX" as never)).toBe("just now");
  });
});
