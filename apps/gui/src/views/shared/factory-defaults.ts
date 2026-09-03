// 出厂默认端点：apps/gui/src-tauri/src/config.rs default_* 的展示层镜像。
// 防漂移：factory-defaults.test.ts 直接解析 config.rs 的 vec! 字面量对表。
// 后端契约：节点装配时空列表回落出厂默认（src-tauri state.rs
// with_factory_fallback），持久层不回写；此处用于空态提示与用户主动点击
// 的恢复入口（仅预填表单）。
export const FACTORY_LIST_DEFAULTS = {
  bootstrap: ["43.240.223.138/u3400", "121.196.193.177/u3400"],
  relayAddrs: ["43.240.223.138/u3403", "121.196.193.177/u3403"],
  observationAddrs: ["121.196.193.177:3402"],
} as const;

export type FactoryListName = keyof typeof FACTORY_LIST_DEFAULTS;

export const FACTORY_LIST_HINT_KEYS = {
  bootstrap: "settings.defaults.bootstrapHint",
  relayAddrs: "settings.defaults.relayHint",
  observationAddrs: "settings.defaults.observationHint",
} as const;

export function factoryRows(name: FactoryListName): { value: string }[] {
  return FACTORY_LIST_DEFAULTS[name].map((value) => ({ value }));
}
