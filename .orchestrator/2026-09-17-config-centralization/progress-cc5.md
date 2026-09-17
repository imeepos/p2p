# CC5 检查点 1（2026-09-17）

- C1 完成：ipc-types/ipc 增 5 命令绑定与类型（契约逐字）；dial-target 导出 isValidPeerId；新增 lib/mock-ftp-peers.ts（内存 mock，密码不回显+空密码保留语义）并装配进 mock-ipc；mock 单测 5 例绿。
- C2 完成：settings.ftp.*/settings.staticPeers.* 命名空间与 common.validation 六个新校验码，zh/en 各 +66 键（1842=1842），check:i18n exit 0。
- C3 完成：FTP 卡接入主表单——config-schema 增 ftp 三字段（root 未装配可空、装配后必填 superRefine；账号行 existing 标记+新用户密码必填+重名拦截）；use-settings-save 双写双读；settings-view 错误路由 services 分节+挂卡；focus-first-error 补定位；ftp-card/账号编辑器组件；渲染矩阵测试 15 例绿（含空密码=保留语义与放弃回滚）。既有 settings-view-nav / settings-focus-error 测试 mock 补齐。附注：FtpConfigSaveInput 类型在本检查点随 config-schema 消费补入 ipc-types。
- C4 完成：静态对端卡（列表/新增/编辑/删除二次确认）+ 独立编辑器（peerId base58 预检复用 isValidPeerId、地址行复用 AddressListEditor、PeerId 编辑锁定）；渲染矩阵 7 例绿。附带修复存量 bug：AddressListEditor 行级校验消息路径（errors.<i>.value.message）此前从未渲染，行内红字现可见。
- C4b：ftp-card.test.tsx 359 行超红线，拆分 unit 矩阵（227 行）与设置页集成（99 行）两文件，均过 typecheck/eslint/vitest。
