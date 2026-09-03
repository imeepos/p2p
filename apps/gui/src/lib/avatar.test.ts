import { describe, expect, it } from "vitest";

import {
  AVATAR_EDGE,
  AVATAR_FILE_MAX_BYTES,
  AvatarFileError,
  coverCrop,
  fileToAvatarDataUrl,
} from "./avatar";

describe("coverCrop", () => {
  it("横图取高度为边、纵图取宽度为边，均居中", () => {
    expect(coverCrop(200, 100)).toEqual({ sx: 50, sy: 0, sw: 100, sh: 100 });
    expect(coverCrop(80, 160)).toEqual({ sx: 0, sy: 40, sw: 80, sh: 80 });
    expect(coverCrop(100, 100)).toEqual({ sx: 0, sy: 0, sw: 100, sh: 100 });
  });
});

describe("fileToAvatarDataUrl 错误路径", () => {
  it("超过原图大小闸门直接拒绝（不解码）", async () => {
    const blob = new Blob(["x".repeat(AVATAR_FILE_MAX_BYTES + 1)]);
    const file = new File([blob], "big.png", { type: "image/png" });
    await expect(fileToAvatarDataUrl(file)).rejects.toMatchObject({
      name: "AvatarFileError",
      code: "avatarTooLarge",
    });
  });

  it("jsdom 无解码器：不可解码文件归为 avatarInvalid", async () => {
    const file = new File(["not-an-image"], "fake.png", { type: "image/png" });
    await expect(fileToAvatarDataUrl(file)).rejects.toBeInstanceOf(AvatarFileError);
  });
});

describe("常量契约", () => {
  it("输出边长 128", () => {
    expect(AVATAR_EDGE).toBe(128);
  });
});
