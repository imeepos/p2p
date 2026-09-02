import { describe, expect, it } from "vitest";

import { parseDialTarget } from "./dial-target";

// 断言集合与 src-tauri/src/proto.rs 的 parse_target 测试同规则族：
// 视图预检放行的，桥接层裁决不应更严地拒绝常见合法形态。
const PEER = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQ";

describe("parseDialTarget", () => {
  it("放行 QUIC/TCP 与裸 IPv6（对齐 proto::parse_target）", () => {
    expect(parseDialTarget(`${PEER}@192.168.1.5/u3400`)).toEqual({
      peerId: PEER,
      addr: "192.168.1.5/u3400",
    });
    expect(parseDialTarget(`${PEER}@192.168.1.5/t3401`)).not.toBeNull();
    expect(parseDialTarget(`${PEER}@::1/t3401`)?.addr).toBe("::1/t3401");
  });

  it("拒绝缺分隔符/缺 u t 前缀/坏类型/坏端口段", () => {
    expect(parseDialTarget("no-separator")).toBeNull();
    expect(parseDialTarget(`${PEER}@1.2.3.4`)).toBeNull();
    expect(parseDialTarget(`${PEER}@1.2.3.4/3400`)).toBeNull();
    expect(parseDialTarget(`${PEER}@1.2.3.4/x3400`)).toBeNull();
    expect(parseDialTarget(`${PEER}@1.2.3.4/u0`)).toBeNull();
    expect(parseDialTarget(`${PEER}@1.2.3.4/u99999`)).toBeNull();
    expect(parseDialTarget(`${PEER}@1.2.3.4/u34.5`)).toBeNull();
  });

  it("拒绝非 base58 与长度不符的 PeerId", () => {
    expect(parseDialTarget("zzz-not-base58@1.2.3.4/u3400")).toBeNull();
    expect(parseDialTarget(`short@1.2.3.4/u3400`)).toBeNull();
    expect(parseDialTarget(`${PEER}extra-long-peer-id-padding-padding@1.2.3.4/u3400`)).toBeNull();
  });
});
