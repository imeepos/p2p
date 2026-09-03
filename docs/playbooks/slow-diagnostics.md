# Playbook: 电脑卡慢诊断

- 名称: 电脑卡慢诊断
- 问题类别: performance-slow
- 推荐 runner:
- 前置条件:
  - Windows 10/11（64 位）
  - 已建立修复会话，诊断步骤 scope 为 diag，修复步骤需 fix
  - 修复助手以管理员权限运行（部分系统查询需要）
- 备注: P0b 真机演练时校准——命令正确性以 Windows 通用知识为准，实机执行后回写校准

## 红线清单

- 禁止 format 或任何低级磁盘操作
- 禁止结束系统关键进程（System、smss 等），结束用户进程前必须先列清单并审批
- 删除任何文件前必须先以只读命令列出目标清单
- 不得修改杀毒软件配置或使其失效

## 步骤 1 采集系统快照

- 说明: 用 sys_snapshot 采集 CPU/内存/磁盘基线，作为后续判断依据。
- 工具: sys_snapshot
- 参数: 无
- 验收: 返回无错误，且包含 CPU、内存、磁盘使用率字段。
- 红线: 纯只读，不修改任何系统状态。

## 步骤 2 查询高 CPU 占用进程

- 说明: 列出按 CPU 时间排序的前 15 个进程，定位卡慢元凶。
- 工具: shell_exec
- 命令: Get-Process | Sort-Object CPU -Descending | Select-Object -First 15 Name, Id, CPU, WorkingSet
- 风险档: read
- 验收: 输出为按 CPU 降序的进程表，包含 Name/Id/CPU/WorkingSet 列。
- 红线: 只读查询，禁止结束任何进程。

## 步骤 3 查询高内存占用进程

- 说明: 列出按工作集内存排序的前 15 个进程，判断内存不足型卡顿。
- 工具: shell_exec
- 命令: Get-Process | Sort-Object WorkingSet -Descending | Select-Object -First 15 Name, Id, WorkingSet
- 风险档: read
- 验收: 输出为按内存降序的进程表。
- 红线: 只读查询，禁止结束任何进程。

## 步骤 4 查询磁盘剩余空间

- 说明: 汇总各固定磁盘的剩余/总量，判断磁盘满导致的卡慢。
- 工具: shell_exec
- 命令: Get-CimInstance Win32_LogicalDisk -Filter "DriveType=3" | Select-Object DeviceID, @{n='FreeGB';e={[math]::Round($_.FreeSpace/1GB,1)}}, @{n='TotalGB';e={[math]::Round($_.Size/1GB,1)}}
- 风险档: read
- 验收: 输出每块固定磁盘的 DeviceID 与 FreeGB/TotalGB。
- 红线: 只读查询，不写入任何文件。
- 备注: P0b 真机演练时校准哈希表格式

## 步骤 5 查询内存总量与可用量

- 说明: 读取物理内存总量与可用量，判断是否需要清理常驻程序或扩容。
- 工具: shell_exec
- 命令: Get-CimInstance Win32_OperatingSystem | Select-Object @{n='TotalGB';e={[math]::Round($_.TotalVisibleMemorySize/1MB,1)}}, @{n='FreeGB';e={[math]::Round($_.FreePhysicalMemory/1MB,1)}}
- 风险档: read
- 验收: 输出 TotalGB 与 FreeGB 两个内存字段。
- 红线: 只读查询。

## 步骤 6 结束确认的高占用用户进程

- 说明: 结合步骤 2/3 结果与客户确认，结束占资源且非必需的用户进程。
- 工具: shell_exec
- 命令: Stop-Process -Id <PID> -Force
- 风险档: write
- 验收: Get-Process -Id <PID> 不再返回该进程（进程已退出）。
- 红线: 仅限已与客户确认的用户进程；系统进程禁止结束；结束前先展示进程清单。
- 备注: <PID> 取步骤 2/3 输出中的实际进程 Id，真机演练时校准参数替换方式

## 步骤 7 清理用户临时文件

- 说明: 删除 %TEMP% 下可删除的用户临时文件，恢复磁盘余量。
- 工具: shell_exec
- 命令: Get-ChildItem $env:TEMP -File -ErrorAction SilentlyContinue | Remove-Item -Force
- 风险档: write
- 验收: %TEMP% 下文件数显著减少，且系统无新增报错。
- 红线: 仅限 $env:TEMP 目录且仅文件（不递归）；删除前先执行只读列表核对；被占用文件由 Remove-Item 自带忽略错误。

## 步骤 8 复查系统快照

- 说明: 修复后再次采集快照，比对修复前后指标变化，确认卡慢缓解。
- 工具: sys_snapshot
- 参数: 无
- 验收: 快照返回无错误，且 CPU/内存/磁盘占用不高于修复前。
- 红线: 纯只读。
