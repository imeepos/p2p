import type { ChatFriendJson } from "@/lib/ipc-types";

// 好友分组纯逻辑（IM-T43）：与 CLI friends.rs group_sections 同口径——
// 组名字典序，未分组(null)恒置底；空串组名视同未分组，不落盘。

// 折叠记忆专用键：前缀避免与用户组名冲突（仅影响折叠记忆，不影响分组语义）。
const UNGROUPED_KEY = "__ungrouped__";
const COLLAPSE_KEY = "chat.friendGroups.collapsed";

export function collapseKeyOf(name: string | null): string {
  return name ?? UNGROUPED_KEY;
}

// 组名归一读取：null/空串/纯空白统一为 null（未分组）。
export function groupNameOf(friend: ChatFriendJson): string | null {
  const name = friend.group?.trim();
  return name ? name : null;
}

export interface FriendGroupSection {
  // null = 未分组虚拟组（置底，不落盘）
  name: string | null;
  friends: ChatFriendJson[];
}

export function groupSections(friends: ChatFriendJson[]): FriendGroupSection[] {
  const names: (string | null)[] = [];
  for (const friend of friends) {
    const name = groupNameOf(friend);
    if (!names.includes(name)) names.push(name);
  }
  const sorted = [...names].sort((a, b) => {
    if (a === null) return 1;
    if (b === null) return -1;
    return a.localeCompare(b);
  });
  return sorted.map((name) => ({
    name,
    friends: friends.filter((friend) => groupNameOf(friend) === name),
  }));
}

// 现有组名清单（移动分组对话框的候选列表），字典序。
export function existingGroupNames(friends: ChatFriendJson[]): string[] {
  return groupSections(friends)
    .map((section) => section.name)
    .filter((name): name is string => name !== null);
}

// 折叠态持久化：localStorage 不可用/损坏时回退全展开并留 warn（不静默）。
export function loadCollapsedGroups(): Set<string> {
  try {
    const raw = window.localStorage.getItem(COLLAPSE_KEY);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? new Set(parsed.filter((x): x is string => typeof x === "string"))
      : new Set();
  } catch (error) {
    console.warn("[chat] 分组折叠态读取失败，按全展开处理", error);
    return new Set();
  }
}

export function saveCollapsedGroups(groups: Set<string>): void {
  try {
    window.localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...groups]));
  } catch (error) {
    console.warn("[chat] 分组折叠态保存失败", error);
  }
}
