# CC1 进度账本（feat/cc-front 独占）

- 2026-09-17 CP1 chore(i18n) 87754f25：settings.remoteAccess.* + fpsRange/loopbackAddrFormat
  双语键落齐；pnpm check:i18n 退出 0（zh=1793 en=1793）。
- 2026-09-17 CP2 feat(settings)：契约三字段进 ipc-types/config-schema 整包往返 +
  远程访问区（卡片/导航/错误分节定位）；红证=改动前往返恰丢三字段（探针 diff 在案），
  绿证=聚焦 vitest 6 文件 35 测试全绿；typecheck 退出 0；改动文件 eslint 退出 0。
- 2026-09-17 CP3 feat(remote-access)：rd 卡 approvalOn/fps 初值改读 GuiConfig
  （useRdCardSettings 挂载 configGet，缺省/读失败回退 true/15 且留 error 日志；
  卡内临时改动随 startHost/rdQualitySet 原语义下发）。聚焦 vitest 2 文件 12 测试绿；
  typecheck 0；改动文件 eslint 0（途中 react-hooks/set-state-in-effect 一红，
  经把回填态收进 hook 异步回调转绿）。
- 2026-09-17 CP4 收尾自验：聚焦 vitest 10 文件 59 测试全绿（含 mock-ipc/save-bar
  保险回归）；typecheck 0；check:i18n PASS（zh=en=1793）；11 改动文件 eslint 0；
  行数自查全部 ≤300、新增函数 ≤60（FpsRow 49/卡体 36）；分支 feat/cc-front
  推送 origin，主干合并按计划留主会话执行。
