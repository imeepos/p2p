// 交互面纯模型单测：权限三态与 60s 倒计时边界、配置整表替换、用量合并。
import { describe, expect, it } from "vitest";

import {
  addPermission,
  allowOptionId,
  applyConfigOptions,
  applyUsage,
  dropInteraction,
  emptyInteraction,
  permissionExpired,
  permissionSecondsLeft,
  rejectUnanswered,
  resolvePermission,
} from "./interaction-model";

function pendingReq(requestId: number, receivedAt = 0) {
  return {
    requestId,
    sessionId: "s-1",
    title: "Run tests",
    toolKind: "execute",
    options: [
      { optionId: "allow-once", name: "Allow", kind: "allow_once" },
      { optionId: "reject-once", name: "Deny", kind: "reject_once" },
    ],
    receivedAt,
  };
}

describe("interaction model", () => {
  it("addPermission 登记 pending，同 id 幂等", () => {
    let st = emptyInteraction();
    st = addPermission(st, pendingReq(7));
    st = addPermission(st, pendingReq(7));
    expect(st.permissions).toHaveLength(1);
    expect(st.permissions[0]).toMatchObject({ requestId: 7, status: "pending" });
  });

  it("allowOptionId 选首个 allow_* 选项，无则 null", () => {
    expect(allowOptionId(pendingReq(1).options)).toBe("allow-once");
    expect(allowOptionId([{ optionId: "r", name: "Deny", kind: "reject_once" }])).toBeNull();
  });

  it("resolvePermission 批准/拒绝，只翻 pending，已决不二次翻转", () => {
    let st = emptyInteraction();
    st = addPermission(st, pendingReq(7));
    st = resolvePermission(st, 7, "approved");
    expect(st.permissions[0].status).toBe("approved");
    st = resolvePermission(st, 7, "rejected");
    expect(st.permissions[0].status).toBe("approved");
  });

  it("60s 倒计时：边界 59999ms 未到，60000ms 归零；秒数向上取整", () => {
    const req = pendingReq(7, 1_000_000);
    expect(permissionExpired(req, 1_000_000 + 59_999)).toBe(false);
    expect(permissionExpired(req, 1_000_000 + 60_000)).toBe(true);
    expect(permissionSecondsLeft(req, 1_000_000 + 1_500)).toBe(59);
    expect(permissionSecondsLeft(req, 1_000_000 + 60_500)).toBe(0);
  });

  it("rejectUnanswered 只翻 pending（一次性 grant 不改已决）", () => {
    let st = emptyInteraction();
    st = addPermission(st, pendingReq(1));
    st = addPermission(st, pendingReq(2));
    st = resolvePermission(st, 1, "approved");
    st = rejectUnanswered(st);
    expect(st.permissions.map((p) => p.status)).toEqual(["approved", "rejected"]);
  });

  it("applyConfigOptions 整表替换，undefined 不动", () => {
    let st = emptyInteraction();
    st = applyConfigOptions(st, [{ id: "model", name: "Model", type: "select", currentValue: "a", options: [] }]);
    st = applyConfigOptions(st, undefined);
    expect(st.configOptions).toHaveLength(1);
    st = applyConfigOptions(st, []);
    expect(st.configOptions).toHaveLength(0);
  });

  it("applyUsage 合并增量并拒绝非法数值", () => {
    let st = emptyInteraction();
    st = applyUsage(st, { used: 100, size: 200 });
    st = applyUsage(st, { used: 150 });
    expect(st.usage).toEqual({ used: 150, size: 200 });
    st = applyUsage(st, { used: -5 });
    expect(st.usage?.used).toBe(150);
    st = applyUsage(st, { used: Number.NaN });
    expect(st.usage?.used).toBe(150);
  });

  it("dropInteraction 移除会话切片", () => {
    let st = emptyInteraction();
    st = addPermission(st, pendingReq(1));
    const map = dropInteraction({ "s-1": st }, "s-1");
    expect("s-1" in map).toBe(false);
  });
});