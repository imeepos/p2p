# IM-T53 chat-boundaries 负载型假红根因复盘（2026-09-05）

## 症状（T43 验收，负载 30-60）
1. empty-state / cancel-pending 用例 5s 超时
2. deduplicates chat_message：期望 2 实得 6
3. app-boot 逐路由用例 33s 级整体超时

## 根因链
- vitest 默认单用例预算 5s。重负载下动态 import("./chat-view")（整棵组件树
  transform）+ 渲染即可超限，用例被判死。
- vitest 无法取消已在跑的 async 用例：判死后后台 continuation 继续，在
  afterEach cleanup 之后才执行到 render(<ChatView />)，游离实例常驻 body。
- 游离实例与后续用例共享同一 zustand store，后续用例事件一到全部同步重渲。
  dedup 用例断言时 body 里有 3 个 ChatView（empty-state/取消占位/本用例），
  每实例渲染气泡 + 侧栏摘要各一处「重复入站」，3 × 2 = 6。
- store 去重本身健全（list.some id 守卫 + mergeMessages byId），
  「期望 2 实得 6」是 DOM 污染，不是状态重复——方向修错会白改 store。

## 修法（fix/im-t53-flake-hardening）
- 逐点给预算，不全局放宽 testTimeout：waitFor 显式 10s；ChatView 用例 20s、
  Composer 用例 15s。用例不再被判死，continuation 污染源头消除。
- dedup 断言改双层：store 层按 id 精确断言（语义）+ message-scroll 容器域内
  DOM 计数（渲染），游离实例即使再出现也不影响结果。
- store 级回归测试锁死「事件先于历史加载」与「历史先落再收重复事件」两条
  时序的 id 唯一性（chat-store.events.test.ts，受控 Deferred 页时序）。
- app-boot：boot 收进 beforeAll（模块只求值一次的约束不变），7 路由拆独立
  用例，单路由 20s 预算互不拖累。

## 可复用经验（待入 skill references/known-issues.md）
- 症状：RTL 用例计数类断言按整数倍翻倍 + 同文件前序用例有超时假红。
- 判定：几乎必是超时 continuation 渲染的游离组件，先查 DOM 实例数再查状态。
- 预防：动态 import 重组件的用例显式给足单用例预算；计数断言用 within()
  限定容器；store 语义与 DOM 渲染分开断言。
