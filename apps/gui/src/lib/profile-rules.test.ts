import { describe, expect, it } from "vitest";

import {
  AVATAR_MAX_LEN,
  DESCRIPTION_MAX_CHARS,
  NAME_MAX_CHARS,
  isValidAvatarDataUrl,
  validateNodeProfile,
} from "./profile-rules";

const PNG_PREFIX = "data:image/png;base64,";
const WEBP_PREFIX = "data:image/webp;base64,";
const PREFIX_LEN = WEBP_PREFIX.length;

describe("isValidAvatarDataUrl", () => {
  it("PNG/JPEG/WebP 白名单内的合法 base64 通过", () => {
    expect(isValidAvatarDataUrl(PNG_PREFIX + "aGVsbG8=")).toBe(true);
    expect(isValidAvatarDataUrl("data:image/jpeg;base64,/9j/4AAQ")).toBe(true);
    expect(isValidAvatarDataUrl(WEBP_PREFIX + "UklGRh4A")).toBe(true);
  });

  it("MIME 白名单外与非 data URL 拒绝", () => {
    expect(isValidAvatarDataUrl("data:image/gif;base64,R0lGOD")).toBe(false);
    expect(isValidAvatarDataUrl("data:image/png")).toBe(false);
    expect(isValidAvatarDataUrl("https://example.com/a.png")).toBe(false);
  });

  it("非法 base64 字符拒绝", () => {
    expect(isValidAvatarDataUrl(PNG_PREFIX + "###")).toBe(false);
  });

  it("超长拒绝，恰达上限通过", () => {
    const atLimit = WEBP_PREFIX + "a".repeat(AVATAR_MAX_LEN - PREFIX_LEN);
    expect(isValidAvatarDataUrl(atLimit)).toBe(true);
    expect(isValidAvatarDataUrl(atLimit + "a")).toBe(false);
  });
});

describe("validateNodeProfile", () => {
  const base = { name: "", description: "", avatar: null };

  it("全空默认通过", () => {
    expect(validateNodeProfile(base)).toBeNull();
  });

  it("name 校验 trim 后长度，超限返回中文错误", () => {
    expect(validateNodeProfile({ ...base, name: " ".repeat(NAME_MAX_CHARS) })).toBeNull();
    expect(validateNodeProfile({ ...base, name: "名".repeat(NAME_MAX_CHARS + 1) })).toMatch(
      /节点名称过长/,
    );
  });

  it("description 超限返回中文错误", () => {
    expect(validateNodeProfile({ ...base, description: "述".repeat(DESCRIPTION_MAX_CHARS + 1) })).toMatch(
      /节点描述过长/,
    );
  });

  it("avatar 非法返回中文错误", () => {
    expect(validateNodeProfile({ ...base, avatar: "data:image/gif;base64,R0lGOD" })).toMatch(
      /头像/,
    );
  });
});
