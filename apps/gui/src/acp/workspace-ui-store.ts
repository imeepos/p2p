// 侧栏两级树的 UI 态（uix-spec §4.2 WorkspaceUiSlice）：独立小 store，防污染
// AcpConsoleState 主表；派生树不落 store（workspace-model 纯函数 + 组件层 selector）。
import { create } from "zustand";

interface WorkspaceUiState {
  /** 用户手动折叠的组键（内存态；含当前会话的组由 isGroupOpen 强制展开） */
  collapsedGroupKeys: string[];
  /** 「展开 N 个」一次性全开的组键（uix-spec I4，再点组头即复位） */
  transientExpandedKeys: string[];
  /** 本地搜索词（uix-spec I8） */
  workspaceQuery: string;
  toggleGroup: (key: string) => void;
  expandGroupTransiently: (key: string) => void;
  setWorkspaceQuery: (query: string) => void;
}

export const useWorkspaceUiStore = create<WorkspaceUiState>()((set) => ({
  collapsedGroupKeys: [],
  transientExpandedKeys: [],
  workspaceQuery: "",
  toggleGroup: (key) =>
    set((s) => ({
      collapsedGroupKeys: s.collapsedGroupKeys.includes(key)
        ? s.collapsedGroupKeys.filter((k) => k !== key)
        : [...s.collapsedGroupKeys, key],
      // 手动开合清掉该组的一次性展开，避免残留态压住后续折叠
      transientExpandedKeys: s.transientExpandedKeys.filter((k) => k !== key),
    })),
  expandGroupTransiently: (key) =>
    set((s) => ({
      transientExpandedKeys: s.transientExpandedKeys.includes(key)
        ? s.transientExpandedKeys
        : [...s.transientExpandedKeys, key],
    })),
  setWorkspaceQuery: (query) => set({ workspaceQuery: query }),
}));

/** 测试隔离：复位侧栏 UI 态 */
export function resetWorkspaceUiForTest(): void {
  useWorkspaceUiStore.setState({
    collapsedGroupKeys: [],
    transientExpandedKeys: [],
    workspaceQuery: "",
  });
}
