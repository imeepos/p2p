# CC1 进度账本（feat/cc-front 独占）

- 2026-09-17 CP1 chore(i18n) 87754f25：settings.remoteAccess.* + fpsRange/loopbackAddrFormat
  双语键落齐；pnpm check:i18n 退出 0（zh=1793 en=1793）。
- 2026-09-17 CP2 feat(settings)：契约三字段进 ipc-types/config-schema 整包往返 +
  远程访问区（卡片/导航/错误分节定位）；红证=改动前往返恰丢三字段（探针 diff 在案），
  绿证=聚焦 vitest 6 文件 35 测试全绿；typecheck 退出 0；改动文件 eslint 退出 0。
