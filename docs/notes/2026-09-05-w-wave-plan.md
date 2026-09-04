# W 波开发计划（2026-09-05，检索定制版）

## 一、输入：待办清单与暴露缺陷
- P1 GUI 不实时感知 CLI 写入（N2 实测：需刷新才可见，契约降级为冷一致）
- P1 GUI 打包分发不成环（updater 插件与 CI 签名流水线已由 IM 线落地，缺本地一键构建+冒烟）
- P2 AI 接入实战未验证（ai-guide/页面协议已交付，无真实 AI 操作者试运行）
- 已修复缺陷存档：真机快照互锁（c76c41a）、并发写静默丢失（3fb0b59）、守卫负载超时（bcaca61）、复用旧实例假红（ea5c11e）

## 二、社区最佳实践检索结论
1. 文件监听刷新（W1）：Rust 侧用 notify + debouncer（防事件风暴），只监听关键数据文件不递归全目录；Rust 经 Tauri emit 定向事件到前端，前端单监听器做定向 store 重载；需处理「自身写回声抑制」（写序号/所有权标记）。参考：outl-desktop fs_watcher.rs、notify-rs 实践（dev.to）。
2. 打包与更新（W2）：tauri build 产出 app/dmg；updater 产物须签名并生成 latest.json（版本/pub_date/platforms/signature），更新时密码学校验。参考：agaric PR#1912（CI 签名+latest.json）、OmniVoice updater 校验提交。本仓库 IM 线已落 updater 插件与 CI 签名（83bac1b），W2 只做本地一键构建+冒烟，不重复造签名。
3. AI 工具面（W3）：MCP 式工具暴露是社区标准，本仓库已有 repair-helper MCP 宿主与 ACP 协议在途；W3 采取「真实 AI 操作者按 ai-guide 试运行」的轻量验证而非新增协议。

## 三、定制方案（卡片）
- W1（P1）GUI 实时感知：src-tauri 新增 watcher 模块（notify+debouncer，监听 friends/config/profile 关键文件）→ 防抖 ≥500ms → emit data-changed{domains} → 前端单监听器定向刷新 + 回写回声抑制；watcher 失败降级可观测不阻断主功能。验收：scripts/ops/cli-live-e2e.sh「CLI 写→≤3s GUI 读回感知」两连绿；N2 契约章升级为实时语义；pnpm/cargo 全绿。
- W2（P1）打包分发流水线：scripts/release/gui-release.sh——版本三处一致（复用 release.sh 口径）→ tauri build 出 app/dmg → 产物存在/大小/Info.plist 版本冒烟 → updater latest.json 校验（有签名验签，无签名显式标注 unsigned）；本地 unsigned 不碰 CI 密钥。验收：脚本两连绿产出产物，末行 W2-RELEASE-OK。
- W3（P2）AI 接入实战试运行：新 AI 会话按 p2pctl-ai-guide + 页面协议执行端到端剧本（起节点→加友→发消息→读页→截图→回归套件），产出摩擦清单回填 ai-guide。验收：docs/notes/ai-pilot-findings.md 在位 + 剧本全通过。

## 四、执行序
W1 ∥ W2（文件域互斥：W1=src-tauri/src watcher 模块+前端监听；W2=scripts/release+tauri.conf bundle 段）→ W3（待 W1/W2 落地后由协调者以子代理执行）。全程 worktree、逐卡机械验收、当天合并、合并即清分支。
