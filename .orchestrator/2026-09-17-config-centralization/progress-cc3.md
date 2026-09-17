# CC3 进度账本（feat/cc3-front 独占）

- 2026-09-17 22:38 worktree 自 origin/main@09174c3c 建立（任务书基线 c5466089 的直接
  后代，含 W1 全部交付；主会话在派发后推进了 origin/main，非冲突）。检查点 0 无阻断、
  无缺上下文：authz_role_list ipc 在位（ipc.ts:229），FactoryDefaultsNotice 原生支持
  bootstrap/relayAddrs，config-schema 三字段 W1 已就位。
- 2026-09-17 23:00 检查点 1（e20bdf9b）：chore(i18n) settings.network.cardTitle/cardHint
  两键 zh/en 同步；check:i18n PASS（zh=1795 en=1795）。键先于 feature 落地，feature
  提交永不引用缺失键。
- 2026-09-17 23:00 检查点 2（01d9f4f2）：feat(gui) 引导与中继卡（bootstrap-relay-card
  154 行）挂 settings 网络分节（NetworkCard 与 AdvertiseCard 之间）；AddressListEditor
  增可选 confirmRemove 闸（bootstrap 删除对齐 discovery 二次确认，存量调用零变化）；
  默认角色 Select 接 useAuthzStore（authz_role_list），空串经 __none__ 哨兵=禁用自动绑；
  advertise-card 过时注释（「设置页不再重复」）同步修正。
- 2026-09-17 23:00 自验全绿：typecheck exit 0；聚焦 vitest 17 文件 76 用例全过
  （settings 全套 + discovery + hardcoded-copy 扫描 + i18n 相关）；改动 7 文件 eslint
  exit 0。取舍：三入口并入现有网络分节而非新导航区（W1 先例 remoteAccess 独立区因
  字段量小不划算）；测试教训：afterEach 重置 zustand store 必须先 cleanup 再重置，
  否则 React 18 unmount 冲刷 pending passive effect 会拿空 store 重跑 load-once。
