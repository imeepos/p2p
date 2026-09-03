//! 白名单闭集数据表（T24 填充，Q7 并集：卡慢/弹窗清理/C 盘空间三类 playbook 命令）。
//!
//! 表结构与 docs/playbooks/ 草案经 repair-playbook 的 shell_union 导出一一对应
//! （数据一致性测试断言两集合相等，草案改动即测试红，机制性防漂移）；
//! 含管道/重定向/命令替换特征的命令属闭集外（remote-support-plan.md §3.4），
//! patterns 置空、不生成运行时规则，判定层见 [crate::whitelist::deny_reason]。
//! 表数据仅供 [builtin] 与文档引用（T28），判定消费 [crate::whitelist::ShellWhitelist]。

use crate::whitelist::{ArgPat, ShellRule, ShellWhitelist};

/// 一条白名单表项：命令原文、argv[0]、允许参数模式、来源 playbook、风险备注。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhitelistEntry {
    /// playbook 命令原文（与 shell_union 并集逐字一致，数据一致性测试锚点）。
    pub command: &'static str,
    /// argv[0] 程序名（裸名或全路径，匹配大小写不敏感）。
    pub program: &'static str,
    /// 允许参数模式描述：\"any\" / \"exact:<v>\" / \"prefix:<v>\"；空 = 复合命令闭集外。
    pub patterns: &'static [&'static str],
    /// 来源 playbook 文件名。
    pub source: &'static str,
    /// 风险备注。
    pub risk_note: &'static str,
}

/// Q7 并集：23 条命令（slow 6 + popup 9 + c-drive 8，跨类重复命令去重）。
pub static WHITELIST_TABLE: &[WhitelistEntry] = &[
    // slow-diagnostics.md（performance-slow）
    WhitelistEntry {
        command: "Get-Process | Sort-Object CPU -Descending | Select-Object -First 15 Name, Id, CPU, WorkingSet",
        program: "Get-Process",
        patterns: &[],
        source: "slow-diagnostics.md",
        risk_note: "read：进程 CPU 排序查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-Process | Sort-Object WorkingSet -Descending | Select-Object -First 15 Name, Id, WorkingSet",
        program: "Get-Process",
        patterns: &[],
        source: "slow-diagnostics.md",
        risk_note: "read：进程内存排序查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-CimInstance Win32_LogicalDisk -Filter \"DriveType=3\" | Select-Object DeviceID, @{n='FreeGB';e={[math]::Round($_.FreeSpace/1GB,1)}}, @{n='TotalGB';e={[math]::Round($_.Size/1GB,1)}}",
        program: "Get-CimInstance",
        patterns: &[],
        source: "slow-diagnostics.md",
        risk_note: "read：磁盘空间查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-CimInstance Win32_OperatingSystem | Select-Object @{n='TotalGB';e={[math]::Round($_.TotalVisibleMemorySize/1MB,1)}}, @{n='FreeGB';e={[math]::Round($_.FreePhysicalMemory/1MB,1)}}",
        program: "Get-CimInstance",
        patterns: &[],
        source: "slow-diagnostics.md",
        risk_note: "read：内存容量查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Stop-Process -Id <PID> -Force",
        program: "Stop-Process",
        patterns: &["exact:-Id", "any", "exact:-Force"],
        source: "slow-diagnostics.md",
        risk_note: "write：结束确认的高占用用户进程",
    },
    WhitelistEntry {
        command: "Get-ChildItem $env:TEMP -File -ErrorAction SilentlyContinue | Remove-Item -Force",
        program: "Get-ChildItem",
        patterns: &[],
        source: "slow-diagnostics.md",
        risk_note: "write：清理用户临时文件（复合命令，闭集外）",
    },
    // popup-malware-cleanup.md（popup-adware）
    WhitelistEntry {
        command: "Get-Process | Where-Object { $_.MainWindowTitle } | Select-Object Name, Id, MainWindowTitle",
        program: "Get-Process",
        patterns: &[],
        source: "popup-malware-cleanup.md",
        risk_note: "read：带窗口进程查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-ItemProperty HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*, HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\* | Select-Object DisplayName, Publisher, InstallDate, UninstallString | Sort-Object DisplayName",
        program: "Get-ItemProperty",
        patterns: &[],
        source: "popup-malware-cleanup.md",
        risk_note: "read：已安装程序清单（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-CimInstance Win32_StartupCommand | Select-Object Name, Command, Location, User",
        program: "Get-CimInstance",
        patterns: &[],
        source: "popup-malware-cleanup.md",
        risk_note: "read：启动项查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-ScheduledTask | Where-Object { $_.State -ne 'Disabled' } | Select-Object TaskName, TaskPath, State | Sort-Object TaskPath",
        program: "Get-ScheduledTask",
        patterns: &[],
        source: "popup-malware-cleanup.md",
        risk_note: "read：计划任务查询（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Stop-Process -Name <进程名> -Force",
        program: "Stop-Process",
        patterns: &["exact:-Name", "any", "exact:-Force"],
        source: "popup-malware-cleanup.md",
        risk_note: "write：结束疑似弹窗进程",
    },
    WhitelistEntry {
        command: "Get-Package -Name <显示名> | Uninstall-Package",
        program: "Get-Package",
        patterns: &[],
        source: "popup-malware-cleanup.md",
        risk_note: "write：卸载已确认流氓软件（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Disable-ScheduledTask -TaskName <任务名> -TaskPath <路径>",
        program: "Disable-ScheduledTask",
        patterns: &["exact:-TaskName", "any", "exact:-TaskPath", "any"],
        source: "popup-malware-cleanup.md",
        risk_note: "write：禁用可疑计划任务",
    },
    WhitelistEntry {
        command: "netsh winhttp show proxy",
        program: "netsh",
        patterns: &["exact:winhttp", "exact:show", "exact:proxy"],
        source: "popup-malware-cleanup.md",
        risk_note: "read：查询 winhttp 代理",
    },
    WhitelistEntry {
        command: "netsh winhttp reset proxy",
        program: "netsh",
        patterns: &["exact:winhttp", "exact:reset", "exact:proxy"],
        source: "popup-malware-cleanup.md",
        risk_note: "write：重置被劫持 winhttp 代理",
    },
    // c-drive-space-cleanup.md（disk-space-low；跨类重复命令已去重）
    WhitelistEntry {
        command: "Get-ChildItem $env:TEMP -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum | Select-Object @{n='SizeMB';e={[math]::Round($_.Sum/1MB,1)}}",
        program: "Get-ChildItem",
        patterns: &[],
        source: "c-drive-space-cleanup.md",
        risk_note: "read：%TEMP% 占用统计（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "(New-Object -ComObject Shell.Application).Namespace(10).Items() | Measure-Object -Property Size -Sum | Select-Object @{n='SizeMB';e={[math]::Round($_.Sum/1MB,1)}}",
        program: "(New-Object",
        patterns: &[],
        source: "c-drive-space-cleanup.md",
        risk_note: "read：回收站占用统计（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Get-ChildItem C:\\Windows\\SoftwareDistribution\\Download -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum | Select-Object @{n='SizeMB';e={[math]::Round($_.Sum/1MB,1)}}",
        program: "Get-ChildItem",
        patterns: &[],
        source: "c-drive-space-cleanup.md",
        risk_note: "read：更新缓存占用统计（复合命令，闭集外）",
    },
    WhitelistEntry {
        command: "Clear-RecycleBin -Force -ErrorAction SilentlyContinue",
        program: "Clear-RecycleBin",
        patterns: &["exact:-Force", "exact:-ErrorAction", "exact:SilentlyContinue"],
        source: "c-drive-space-cleanup.md",
        risk_note: "write：清空回收站（清空不可恢复，需审批）",
    },
    WhitelistEntry {
        command: "Stop-Service wuauserv -Force",
        program: "Stop-Service",
        patterns: &["exact:wuauserv", "exact:-Force"],
        source: "c-drive-space-cleanup.md",
        risk_note: "write：停止 Windows Update 服务",
    },
    WhitelistEntry {
        command: "Remove-Item C:\\Windows\\SoftwareDistribution\\Download\\* -Recurse -Force -ErrorAction SilentlyContinue",
        program: "Remove-Item",
        patterns: &["any", "exact:-Recurse", "exact:-Force", "exact:-ErrorAction", "exact:SilentlyContinue"],
        source: "c-drive-space-cleanup.md",
        risk_note: "danger：清理更新下载缓存（系统目录写，仅限该路径）",
    },
    WhitelistEntry {
        command: "Start-Service wuauserv",
        program: "Start-Service",
        patterns: &["exact:wuauserv"],
        source: "c-drive-space-cleanup.md",
        risk_note: "write：恢复 Windows Update 服务",
    },
    WhitelistEntry {
        command: "Dism.exe /Online /Cleanup-Image /StartComponentCleanup",
        program: "Dism.exe",
        patterns: &["exact:/Online", "exact:/Cleanup-Image", "exact:/StartComponentCleanup"],
        source: "c-drive-space-cleanup.md",
        risk_note: "write：DISM 清理旧组件（耗时，需告知客户）",
    },
];

/// 解析单条模式描述；非法描述返回 None（表数据受一致性测试约束，兜底拒规则）。
fn parse_pattern(spec: &str) -> Option<ArgPat> {
    match spec {
        "any" => Some(ArgPat::Any),
        _ => spec
            .strip_prefix("exact:")
            .map(ArgPat::exact)
            .or_else(|| spec.strip_prefix("prefix:").map(ArgPat::prefix)),
    }
}

/// 由数据表构建运行时白名单：patterns 为空的复合命令不生成规则（闭集外）。
/// 非法模式描述留告警日志并跳过该条（失败路径可观测，不 panic）。
pub fn builtin() -> ShellWhitelist {
    let mut w = ShellWhitelist::empty();
    for e in WHITELIST_TABLE {
        if e.patterns.is_empty() {
            continue;
        }
        let mut args = Vec::with_capacity(e.patterns.len());
        for spec in e.patterns {
            match parse_pattern(spec) {
                Some(p) => args.push(p),
                None => {
                    tracing::warn!(
                        command = e.command,
                        spec,
                        "白名单表项参数模式非法，跳过该规则"
                    );
                    args.clear();
                    break;
                }
            }
        }
        if !args.is_empty() {
            w.add(ShellRule::new(e.program, args));
        }
    }
    w
}
