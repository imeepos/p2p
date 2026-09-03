import { beforeEach, describe, expect, it } from "vitest";

import { mockUpdateCheck, mockUpdateFixture, mockUpdateOpenReleasePage } from "./mock-update";

beforeEach(() => {
  mockUpdateFixture.scenario = "available";
  mockUpdateFixture.lastOpenedUrl = null;
});

describe("mock-update 三态夹具", () => {
  it("默认有更新：版本高于 0.1.0 且附带完整发布信息", async () => {
    const result = await mockUpdateCheck();
    expect(mockUpdateFixture.scenario).toBe("available");
    expect(result.currentVersion).toBe("0.1.0");
    expect(result.hasUpdate).toBe(true);
    expect(result.latestVersion).not.toBeNull();
    const [major, minor, patch] = result.latestVersion!.split(".").map(Number);
    const current = [0, 1, 0];
    expect(
      major !== current[0] ||
        minor !== current[1] ||
        patch !== current[2],
    ).toBe(true);
    expect(result.releaseUrl).toMatch(/^https:\/\/github\.com\//);
    expect(result.releaseName).toBeTruthy();
    expect(result.releaseNotesMd!.length).toBeGreaterThan(20);
    expect(result.publishedAtMs).toBeLessThan(Date.now());
    expect(result.checkedAtMs).toBeLessThanOrEqual(Date.now());
  });

  it("无更新：latestVersion 等于当前版本且 hasUpdate 为 false", async () => {
    mockUpdateFixture.scenario = "upToDate";
    const result = await mockUpdateCheck();
    expect(result.hasUpdate).toBe(false);
    expect(result.latestVersion).toBe(result.currentVersion);
    expect(result.releaseUrl).toBeNull();
  });

  it("检查失败：抛可读 Err，契约失败语义（禁止静默吞）", async () => {
    mockUpdateFixture.scenario = "failed";
    await expect(mockUpdateCheck()).rejects.toThrow(/更新检查失败/);
  });

  it("打开发布页白名单：https+github.com 放行并记录，其余拒绝", async () => {
    const ok =
      "https://github.com/imeepos/p2p/releases/tag/client-v0.2.0";
    await mockUpdateOpenReleasePage(ok);
    expect(mockUpdateFixture.lastOpenedUrl).toBe(ok);

    await expect(
      mockUpdateOpenReleasePage("http://github.com/imeepos/p2p/releases"),
    ).rejects.toThrow(/https/);
    await expect(
      mockUpdateOpenReleasePage("https://evil.example.com/release"),
    ).rejects.toThrow(/github\.com/);
    await expect(mockUpdateOpenReleasePage("not-a-url")).rejects.toThrow(
      /url 非法/,
    );
    expect(mockUpdateFixture.lastOpenedUrl).toBe(ok);
  });
});
