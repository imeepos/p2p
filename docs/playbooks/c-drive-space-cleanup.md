# Playbook: C 盘空间清理

- 名称: C 盘空间清理
- 问题类别: disk-space-low
- 推荐 runner:
- 前置条件:
  - Windows 10/11（64 位）
  - 修复会话 scope 为 fix（含清理/删除步骤）
  - 修复助手以管理员权限运行（涉及系统目录清理）
- 备注: P0b 真机演练时校准——命令正确性以 Windows 通用知识为准，系统目录清理需确认真机行为

## 红线清单

- 禁止删除个人文件（文档/照片/下载等），仅清理系统明确可回收空间
- 删除任何文件前必须先以只读命令列出目标清单并审批
- 禁止 format 或任何低级磁盘操作
- 系统目录（SoftwareDistribution 等）清理须先停对应服务并在完成后恢复

## 步骤 1 查询各磁盘剩余空间

- 说明: 汇总各固定磁盘剩余空间，确认 C 盘是否紧张及紧张程度。
- 工具: shell_exec
- 命令: Get-CimInstance Win32_LogicalDisk -Filter "DriveType=3" | Select-Object DeviceID, @{n='FreeGB';e={[math]::Round($_.FreeSpace/1GB,1)}}, @{n='TotalGB';e={[math]::Round($_.Size/1GB,1)}}
- 风险档: read
- 验收: 输出每块固定磁盘的 FreeGB/TotalGB。
- 红线: 只读查询。

## 步骤 2 统计用户临时目录占用

- 说明: 统计 %TEMP% 目录总占用，评估可回收量。
- 工具: shell_exec
- 命令: Get-ChildItem $env:TEMP -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum | Select-Object @{n='SizeMB';e={[math]::Round($_.Sum/1MB,1)}}
- 风险档: read
- 验收: 输出 %TEMP% 总占用 SizeMB。
- 红线: 只读统计，不删除任何文件。

## 步骤 3 统计回收站占用

- 说明: 统计回收站总占用，作为可回收项候选。
- 工具: shell_exec
- 命令: (New-Object -ComObject Shell.Application).Namespace(10).Items() | Measure-Object -Property Size -Sum | Select-Object @{n='SizeMB';e={[math]::Round($_.Sum/1MB,1)}}
- 风险档: read
- 验收: 输出回收站占用 SizeMB。
- 红线: 只读统计，不执行清空。

## 步骤 4 统计 Windows Update 缓存占用

- 说明: 统计 SoftwareDistribution\Download 目录占用，评估系统回收量。
- 工具: shell_exec
- 命令: Get-ChildItem C:\Windows\SoftwareDistribution\Download -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum | Select-Object @{n='SizeMB';e={[math]::Round($_.Sum/1MB,1)}}
- 风险档: read
- 验收: 输出该目录占用 SizeMB。
- 红线: 只读统计；本步骤禁止改动系统目录。

## 步骤 5 清空回收站

- 说明: 在步骤 3 确认无重要文件后清空回收站。
- 工具: shell_exec
- 命令: Clear-RecycleBin -Force -ErrorAction SilentlyContinue
- 风险档: write
- 验收: 步骤 3 命令输出 SizeMB 为 0。
- 红线: 清空前必须展示回收站内容清单并获客户确认；回收站清空不可恢复。

## 步骤 6 清理用户临时文件

- 说明: 删除 %TEMP% 目录下文件（非递归，仅文件），回收用户临时文件占用。
- 工具: shell_exec
- 命令: Get-ChildItem $env:TEMP -File -ErrorAction SilentlyContinue | Remove-Item -Force
- 风险档: write
- 验收: 步骤 2 命令输出 SizeMB 显著下降。
- 红线: 仅限 $env:TEMP 且仅文件（不递归）；删除前列表核对；被占用文件自动跳过。

## 步骤 7 停止 Windows Update 服务

- 说明: 停止 wuauserv 服务，为清理更新缓存做准备。
- 工具: shell_exec
- 命令: Stop-Service wuauserv -Force
- 风险档: write
- 验收: Get-Service wuauserv 的 Status 为 Stopped。
- 红线: 仅限 Windows Update 服务；清理完成后必须恢复启动；执行前告知客户服务短暂中断。

## 步骤 8 清理 Windows Update 下载缓存

- 说明: 删除 SoftwareDistribution\Download 下的下载缓存（wuauserv 已停止）。
- 工具: shell_exec
- 命令: Remove-Item C:\Windows\SoftwareDistribution\Download\* -Recurse -Force -ErrorAction SilentlyContinue
- 风险档: danger
- 验收: 步骤 4 命令输出 SizeMB 接近 0。
- 红线: 仅限该缓存目录；属 system 目录写操作，删除前列表核对；本步不可在服务运行中执行。

## 步骤 9 恢复 Windows Update 服务

- 说明: 重新启动 wuauserv 服务，恢复系统更新能力。
- 工具: shell_exec
- 命令: Start-Service wuauserv
- 风险档: write
- 验收: Get-Service wuauserv 的 Status 为 Running。
- 红线: 若启动失败必须告警并上抛，禁止静默忽略。

## 步骤 10 清理旧版本 Windows 组件

- 说明: 用 DISM 清理无用的旧版本组件（耗时较长，属系统回收最大单项）。
- 工具: shell_exec
- 命令: Dism.exe /Online /Cleanup-Image /StartComponentCleanup
- 风险档: write
- 验收: 命令以 0 退出码结束且输出提示操作完成。
- 红线: 需要管理员权限；执行前告知客户耗时与期间勿重启；执行中禁止中断。
