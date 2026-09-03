// 出厂默认端点：apps/gui/src-tauri/src/config.rs default_* 的展示层镜像。
// 后端契约：用户显式清空的列表不会被出厂默认覆盖；此处仅用于空态提示与
// 用户主动点击的恢复入口，禁止自动写回表单或持久层。
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
