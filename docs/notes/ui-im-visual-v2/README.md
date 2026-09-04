# IM-V2 五页视觉二轮修复 前后对比截图留档

分支 feat/im-visual-v2；无头 Chrome 1440x900（VITE_MOCK_IPC=1，dev 端口 5212）。

## 路径清单

| 页面 | 修复前 | 修复后 |
|---|---|---|
| 仪表盘 (#/) | before-dashboard.png | after-dashboard.png |
| 仪表盘运行态 | before-dashboard-running.png | -（运行态证据以 CDP DOM 断言为准） |
| 对端 (#/peers) | before-peers.png | after-peers.png |
| 发现 (#/discovery) | before-discovery.png | after-discovery.png |
| 中继 (#/relay) | before-relay.png | after-relay.png |
| 设置 (#/settings) | before-settings.png | after-settings.png |

## 与识图复核清单的对应

- 仪表盘：D1 顶栏停止按钮中性边框化（运行态红色系仅存于二次确认弹框）；
  D2 两行指标卡统一最小高度（八卡 computed minHeight 112px）；
  D3 趋势空态占位收紧 py-5；D4 底部双卡 min-h-40 与趋势卡 160px 等高。
- 对端：P1 空态卡 max-w-sm+p-6（宽 448 -> 384）；
  P2 筛选 Tab 选中实心填充 + 未选中弱化。
- 发现：F1 左右卡 h-full 底端对齐（实测 334/334）；
  F2 删除按钮 size-9（36px）垂直居中。
- 中继：R2 水位卡分立嵌套边框小卡 + text-lg，左右卡等高（339/339）。
- 设置：S1 头像行 items-center + 说明文字 gray-600（7.56:1 AA）；
  S2 未选中 chip gray-300 描边；S4 mDNS 描述 max-w-sm/leading-5/flex-1；
  S6 保存条提示同行 + AA 对比度。

before 截图为修复前 main（a9ac2e8+c053e36 之后状态），CDP 采集同时段；
after 截图用任务书给定的 headless 命令采集（--virtual-time-budget=15000）。
逐项 DOM/类名级断言见 apps/gui/src/views/im-visual-v2.test.tsx（13 用例）。

## R3 追加（二轮识图复核 7.2/10 打回，5 项修复）

r3/after-*.png 为第三轮修复后五页截图（同 1440x900 命令采集）：

- D3 趋势卡：mock 停止态全零采样点改走紧凑占位（此前零平线空图，
  占位文案从未渲染）。截图中趋势卡 174px 带「暂无趋势数据」。
- D4 底部双卡：min-h-56=224px + 内容垂直居中；像素扫描底卡带
  642-866，与状态栏间无死白。
- R2 中继：footer 钉底 flex-1 + 水位盒拉伸填满，CDP 实测 footer 底
  462 == 水位盒底 462（内容底边像素级重合），卡框 339/339 等高。
- P1 对端空态：去全宽 Card 外壳，空态页 data-slot=card 数量 0。
- S1 头像行：圆标与标签/按钮行同轴（中心 685==685），说明文字整行
  下移，AA 配色维持。
