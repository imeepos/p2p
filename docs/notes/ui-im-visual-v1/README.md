# IM-V1 五页视觉修复 前后对比截图留档

分支 feat/im-visual-v1；无头 Chrome 1440x900（VITE_MOCK_IPC=1，dev 端口 5211）。

## 路径清单

| 页面 | 修复前 | 修复后 |
|---|---|---|
| 仪表盘 (#/) | before-dashboard.png | after-dashboard.png |
| 对端 (#/peers) | before-peers.png | after-peers.png |
| 发现 (#/discovery) | before-discovery.png | after-discovery.png |
| 中继 (#/relay) | before-relay.png | after-relay.png |
| 设置 (#/settings) | before-settings.png | after-settings.png |

副本同存 /tmp/v1-before-*.png、/tmp/v1-after-*.png（临时区，以本目录为准）。

## 逐页修复项（对照识图审计清单）

- 仪表盘：停止节点中性边框化；运行状态 StatusBadge 语义化；两行指标卡
  统一最小高度；底部双卡等高失衡处理；趋势卡空态占位已在位复核。
- 对端：空态卡限宽居中 + 主操作实心；筛选 Tab 激活态描边指示；
  搜索框放大镜图标 + h-9 统一。
- 发现：mDNS 卡运行中绿色徽章（success 语义）+ 详情占位补高度；
  地址删除按钮 size-8（32px 触控下限）；空态走统一 EmptyState。
- 中继：降级链数字徽章步骤条；会话水位 text-2xl semibold；逐跳统计
  h-10/divide-y/空态灰斜体；保存按钮入 border-t footer。
- 设置：头像区对齐复核 + 说明文字对比度；主题/语言 chip 未选中描边；
  mDNS 长描述 max-w 限宽；保存条提示文字对比度 AA。
- 跨页一致性：EmptyState 统一（max-w/图标/主操作居中）；
  StatusBadge 四档语义一处落地；页面标题区已统一走 PageHeader（复核确认）。

审计项 P4（对端页副标题错别字「已知的回话节点」）在开工树上已为
「已知邻居节点与连接管理」，无错别字可修；验收 grep「会话」在位。
「添加新地址」按钮开工树已是 outline 次级样式，截图复核确认保持。
