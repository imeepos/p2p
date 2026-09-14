// 分组纯函数单测（uix-spec I1/I3/I4/I8 + §4.2）：组序/未分组末位/规范化/
// 限流/过滤/开合派生的红绿双向覆盖。
import { describe, expect, it } from "vitest";

import {
  COLLAPSED_SESSION_LIMIT,
  collapseGroupRows,
  filterSessionsByQuery,
  groupKeyOf,
  groupSessionsByWorkspace,
  isGroupOpen,
  normalizeCwd,
  workspaceLabelOf,
  type WorkspaceGroupNode,
} from "./workspace-model";

function sessionOf(sessionId: string, cwd?: string, title?: string) {
  return { sessionId, cwd, title };
}

function keysOf(groups: WorkspaceGroupNode[]): string[] {
  return groups.map((group) => group.key);
}

describe("groupSessionsByWorkspace", () => {
  it("多工作区分组，无 cwd 会话落「未分组」且恒排最后（I1）", () => {
    const groups = groupSessionsByWorkspace(
      [
        sessionOf("s-1", "/tmp/alpha"),
        sessionOf("s-2"),
        sessionOf("s-3", "/tmp/beta"),
        sessionOf("s-4", "/tmp/alpha"),
      ],
      null,
    );
    expect(keysOf(groups)).toEqual(["/tmp/alpha", "/tmp/beta", ""]);
    expect(groups[0].sessions.map((s) => s.sessionId)).toEqual(["s-1", "s-4"]);
    expect(groups[2].sessions.map((s) => s.sessionId)).toEqual(["s-2"]);
  });

  it("空串/纯空白/缺失 cwd 等价归未分组，单会话成组", () => {
    const groups = groupSessionsByWorkspace(
      [sessionOf("s-1", ""), sessionOf("s-2", "   "), sessionOf("s-3")],
      null,
    );
    expect(keysOf(groups)).toEqual([""]);
    expect(groups[0].sessions).toHaveLength(3);
    expect(groups[0].cwd).toBeNull();
  });

  it("组序按 cwd 首次出现序稳定排列", () => {
    const groups = groupSessionsByWorkspace(
      [sessionOf("s-1", "/b"), sessionOf("s-2", "/a"), sessionOf("s-3", "/b"), sessionOf("s-4", "/c")],
      null,
    );
    expect(keysOf(groups)).toEqual(["/b", "/a", "/c"]);
  });

  it("cwd 规范化：尾斜杠同组、组名取 basename、根路径不与未分组相撞", () => {
    const groups = groupSessionsByWorkspace(
      [sessionOf("s-1", "/tmp/alpha/"), sessionOf("s-2", "/tmp/alpha"), sessionOf("s-3", "/")],
      null,
    );
    expect(keysOf(groups)).toEqual(["/tmp/alpha", "/"]);
    expect(groups[0].label).toBe("alpha");
    expect(groups[1].label).toBe("/");
    expect(groupKeyOf(undefined)).toBe("");
  });

  it("containsCurrent 只标当前会话所在组（I2/I3 依据）", () => {
    const groups = groupSessionsByWorkspace(
      [sessionOf("s-1", "/a"), sessionOf("s-2", "/b")],
      "s-2",
    );
    expect(groups[0].containsCurrent).toBe(false);
    expect(groups[1].containsCurrent).toBe(true);
  });
});

describe("collapseGroupRows", () => {
  it("不超限全显且 hiddenCount=0；超限截断并计数（I4）", () => {
    const three = [sessionOf("s-1"), sessionOf("s-2"), sessionOf("s-3")];
    expect(collapseGroupRows(three, { limit: COLLAPSED_SESSION_LIMIT })).toEqual({
      rows: three,
      hiddenCount: 0,
    });
    const seven = Array.from({ length: 7 }, (_, i) => sessionOf("s-" + i));
    const limited = collapseGroupRows(seven, { limit: 5 });
    expect(limited.rows).toHaveLength(5);
    expect(limited.hiddenCount).toBe(2);
  });

  it("limit=0 边界：全隐藏且计数=总数", () => {
    const limited = collapseGroupRows([sessionOf("s-1")], { limit: 0 });
    expect(limited.rows).toHaveLength(0);
    expect(limited.hiddenCount).toBe(1);
  });
});

describe("filterSessionsByQuery", () => {
  it("trim+lowercase 匹配标题/组名/会话 id；空查询返回原序全量（I8）", () => {
    const sessions = [
      sessionOf("id-alpha", "/tmp/one", "Deploy Bot"),
      sessionOf("id-beta", "/tmp/two", "Monitor"),
    ];
    expect(filterSessionsByQuery(sessions, "  ")).toHaveLength(2);
    expect(filterSessionsByQuery(sessions, "deploy")).toEqual([sessions[0]]);
    expect(filterSessionsByQuery(sessions, "TWO")).toEqual([sessions[1]]);
    expect(filterSessionsByQuery(sessions, "id-alpha")).toEqual([sessions[0]]);
    expect(filterSessionsByQuery(sessions, "nomatch")).toEqual([]);
  });
});

describe("isGroupOpen", () => {
  it("含当前会话强制展开；其余由折叠清单决定（I3）", () => {
    expect(isGroupOpen("/a", true, ["/a"])).toBe(true);
    expect(isGroupOpen("/a", false, ["/a"])).toBe(false);
    expect(isGroupOpen("/a", false, [])).toBe(true);
  });
});

describe("workspaceLabelOf/normalizeCwd", () => {
  it("basename 提取与空值回退", () => {
    expect(workspaceLabelOf("/Users/me/ext512/p2p")).toBe("p2p");
    expect(workspaceLabelOf("/tmp/x/")).toBe("x");
    expect(workspaceLabelOf(undefined)).toBe("");
    expect(normalizeCwd(" /tmp/x// ")).toBe("/tmp/x");
    expect(normalizeCwd(null)).toBeNull();
  });
});
