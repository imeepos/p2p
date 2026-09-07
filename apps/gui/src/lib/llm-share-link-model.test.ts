import { describe, expect, it } from "vitest";

import {
  buildLlmShareLink,
  findLlmShareLinkInText,
  LLM_SHARE_LINK_PREFIX,
  LlmShareLinkError,
  parseLlmShareLink,
} from "./llm-share-link-model";

const TOKEN = "0123456789abcdef0123456789abcdef";

function linkOf(overrides: Partial<Record<string, string>> = {}): string {
  const params = new URLSearchParams({
    peer: "7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk",
    token: TOKEN,
    exp: "1788549300",
    sid: "sh-1",
    models: "gpt-4o,deepseek-v3",
    ...overrides,
  });
  return LLM_SHARE_LINK_PREFIX + params.toString();
}

describe("parseLlmShareLink（§5.4 冻结契约解析边界）", () => {
  it("合法链接完整解析：peer/token/exp/sid/models/addrs", () => {
    const link =
      LLM_SHARE_LINK_PREFIX +
      "peer=P1&token=" + TOKEN + "&addr=/ip4/1.2.3.4/udp/4242&exp=1788549300&sid=sh-9&models=a,b";
    const parsed = parseLlmShareLink(link);
    expect(parsed.peer).toBe("P1");
    expect(parsed.token).toBe(TOKEN);
    expect(parsed.addrs).toEqual(["/ip4/1.2.3.4/udp/4242"]);
    expect(parsed.expUnix).toBe(1788549300);
    expect(parsed.sid).toBe("sh-9");
    expect(parsed.models).toEqual(["a", "b"]);
  });

  it("scheme 不符拒绝（含 ACP 前缀不误认）", () => {
    expect(() => parseLlmShareLink("dsh-acp-share://v1?peer=x&token=" + TOKEN)).toThrow(LlmShareLinkError);
    expect(() => parseLlmShareLink("https://example.com/x")).toThrow(LlmShareLinkError);
  });

  it("peer/token 缺失拒绝", () => {
    expect(() => parseLlmShareLink(linkOf({ peer: "" }))).toThrow(/missing/);
    expect(() => parseLlmShareLink(linkOf({ token: "" }))).toThrow(/missing/);
  });

  it("token 非 32-hex 拒绝（严格小写，对齐 ACP 先例）", () => {
    expect(() => parseLlmShareLink(linkOf({ token: "zz" }))).toThrow(/token/);
    expect(() => parseLlmShareLink(linkOf({ token: "z".repeat(32) }))).toThrow(/token/);
    expect(() => parseLlmShareLink(linkOf({ token: TOKEN.toUpperCase() }))).toThrow(/token/);
  });

  it("未知参数忽略；exp 非数字回落 null；addr 可重复", () => {
    const link =
      LLM_SHARE_LINK_PREFIX +
      "peer=P&token=" + TOKEN + "&addr=a&addr=b&exp=abc&models=m1&extra=ignored";
    const parsed = parseLlmShareLink(link);
    expect(parsed.addrs).toEqual(["a", "b"]);
    expect(parsed.expUnix).toBeNull();
    expect(parsed.sid).toBeNull();
  });

  it("models 缺省/空串 → 空数组（不静默造默认值）", () => {
    expect(parseLlmShareLink(linkOf({ models: "" })).models).toEqual([]);
    expect(parseLlmShareLink(linkOf({ models: " , " })).models).toEqual([]);
  });
});

describe("findLlmShareLinkInText（聊天正文识别，参数化 scheme）", () => {
  it("正文内第一条链接被提取，其余文字保留由渲染层处理", () => {
    const link = linkOf();
    expect(findLlmShareLinkInText("看看这个 " + link + " 好用吗")).toBe(link);
  });

  it("无链接返回 null；ACP 链接不误命中 llm-share 识别器", () => {
    expect(findLlmShareLinkInText("普通文本")).toBeNull();
    const acp = "dsh-acp-share://v1?peer=p&token=" + TOKEN;
    expect(findLlmShareLinkInText(acp)).toBeNull();
  });

  it("链接到空白/引号/尖括号为止", () => {
    const link = linkOf();
    expect(findLlmShareLinkInText(link + " 结束")).toBe(link);
    expect(findLlmShareLinkInText('"' + link + '"')).toBe(link);
    expect(findLlmShareLinkInText("<" + link + ">")).toBe(link);
  });
});

describe("buildLlmShareLink（mock 出借侧拼装）", () => {
  it("roundtrip：build → parse 还原全部字段", () => {
    const link = buildLlmShareLink({
      peer: "P1",
      addrs: ["/ip4/a"],
      token: TOKEN,
      expUnix: 1788549300,
      sid: "sh-1",
      models: ["gpt-4o", "deepseek-v3"],
    });
    const parsed = parseLlmShareLink(link);
    expect(parsed).toEqual({
      peer: "P1",
      addrs: ["/ip4/a"],
      token: TOKEN,
      expUnix: 1788549300,
      sid: "sh-1",
      models: ["gpt-4o", "deepseek-v3"],
    });
  });
});
