//! 红线清单：无条件拒、无开关、不可配置绕过。
//!
//! 语义（remote-support-plan.md §3.4）：以下五类操作无论 scope、无论审批，
//! 一律直接拒绝——format/低级磁盘操作、触碰密码凭据文件、加密用户文件、
//! 批量删除（单调用多路径或递归删用户目录）、使杀毒软件失效。
//!
//! 判定形如"关键词表 + 匹配原语"，全部数据化承载于本模块常量；
//! 匹配大小写不敏感、路径先归一（[crate::util::norm_path]），
//! 任何变形绕过（大小写/分隔符/".." 折叠/设备前缀）都归一到同一判定面。

use crate::risk::ToolCall;
use crate::util;

/// 五条红线枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redline {
    /// format / 低级磁盘操作。
    FormatDisk,
    /// 触碰密码凭据文件。
    Credentials,
    /// 加密用户文件。
    EncryptUserFiles,
    /// 批量删除（单调用多路径或递归删用户目录）。
    BatchDelete,
    /// 使杀毒软件失效。
    DisableAntivirus,
}

impl Redline {
    /// 机器可读名称（审计记录用）。
    pub fn name(&self) -> &'static str {
        match self {
            Self::FormatDisk => "format_disk",
            Self::Credentials => "credentials",
            Self::EncryptUserFiles => "encrypt_user_files",
            Self::BatchDelete => "batch_delete",
            Self::DisableAntivirus => "disable_antivirus",
        }
    }

    /// 人类可读说明。
    pub fn reason(&self) -> &'static str {
        match self {
            Self::FormatDisk => "format 或低级磁盘操作，无条件拒绝",
            Self::Credentials => "触碰密码/凭据/密钥文件，无条件拒绝",
            Self::EncryptUserFiles => "加密用户文件，无条件拒绝",
            Self::BatchDelete => "批量删除（多路径或递归删用户目录），无条件拒绝",
            Self::DisableAntivirus => "使杀毒软件失效，无条件拒绝",
        }
    }
}

/// 对一次工具调用做红线判定；未命中返回 None。
pub fn check_tool_call(call: &ToolCall) -> Option<Redline> {
    match call.tool.as_str() {
        "shell_exec" => check_command(&call.shell_argv()),
        "fs_read" | "fs_write" | "fs_edit" | "fs_search" | "fs_list" | "fs_delete" => {
            let paths = [
                call.param("path").unwrap_or(""),
                call.param("paths").unwrap_or(""),
            ];
            if paths
                .iter()
                .any(|p| !p.is_empty() && is_credentials_path(p))
            {
                return Some(Redline::Credentials);
            }
            if call.tool == "fs_delete" {
                return check_fs_delete(call);
            }
            None
        }
        _ => None,
    }
}

/// shell 命令行红线判定：argv 数组依次检查五类红线。判定顺序即优先级。
pub fn check_command(argv: &[String]) -> Option<Redline> {
    let line = argv.join(" ");
    // 加密判定先于 format：cryptsetup luksFormat 等兼有格式化语义的操作
    // 归入加密红线（同为无条件拒，仅影响记录的原因）
    if util::contains_any(&line, ENCRYPT_KEYWORDS) {
        return Some(Redline::EncryptUserFiles);
    }
    if util::contains_any(&line, FORMAT_KEYWORDS) {
        return Some(Redline::FormatDisk);
    }
    if check_antivirus(&line) {
        return Some(Redline::DisableAntivirus);
    }
    if is_credentials_input(&line) {
        return Some(Redline::Credentials);
    }
    if check_batch_delete(argv) {
        return Some(Redline::BatchDelete);
    }
    None
}

fn check_antivirus(line: &str) -> bool {
    util::contains_any(line, ANTIVIRUS_NAMES) && util::contains_any(line, ANTIVIRUS_ACTIONS)
}

fn check_batch_delete(argv: &[String]) -> bool {
    if argv.is_empty() || !util::contains_any(&argv[0], DELETE_COMMANDS) {
        return false;
    }
    let (operands, recursive) = collect_operands(&argv[1..]);
    operands.len() >= 2 || (recursive && operands.len() == 1 && is_guard_delete_path(&operands[0]))
}

/// 取命令行操作数并标记是否递归删除。
fn collect_operands(tokens: &[String]) -> (Vec<String>, bool) {
    let mut operands = Vec::new();
    let mut recursive = false;
    let mut after_dashdash = false;
    for t in tokens {
        if !after_dashdash && t == "--" {
            after_dashdash = true;
            continue;
        }
        // 旗标：'-' 前缀（Unix）或精确 "/s"（Windows 递归开关）；
        // 其余以 '/' 开头的 token 是绝对路径操作数，不得吞掉
        if !after_dashdash && (t.starts_with('-') || t == "/s") {
            if util::contains_any(t, RECURSIVE_FLAGS) {
                recursive = true;
            }
            continue;
        }
        operands.push(t.clone());
    }
    (operands, recursive)
}

/// 递归删除目标是否为"用户目录保护区"：根/盘符根、用户主目录、
/// 桌面/文档/下载/图片/音乐/视频等顶层用户文件夹。
fn is_guard_delete_path(path: &str) -> bool {
    let n = util::norm_path(path);
    let mut segs: Vec<&str> = n.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return true;
    }
    if segs[0].ends_with(':') {
        if segs.len() == 1 {
            return true;
        }
        segs = segs[1..].to_vec();
    }
    if segs.is_empty() {
        return true;
    }
    // "~" 等价于用户主目录前缀
    if segs[0] == "~" {
        segs = segs[1..].to_vec();
    }
    if segs.is_empty() {
        return true;
    }
    let head = segs[0];
    if head == "users" || head == "home" {
        return segs.len() == 1
            || segs.len() == 2
            || (segs.len() == 3 && util::contains_any(segs[2], USER_TOP_DIRS));
    }
    segs.len() == 1 && util::contains_any(segs[0], USER_TOP_DIRS)
}

/// 路径文本是否命中凭据模式（段级 + 整串多词组）。
pub fn is_credentials_path(path: &str) -> bool {
    is_credentials_input(path)
}

/// 任意文本（路径或命令行）是否命中凭据模式。
fn is_credentials_input(text: &str) -> bool {
    let n = util::norm_path(text);
    if util::contains_any(&n, CREDENTIAL_MULTIWORD) {
        return true;
    }
    n.split('/')
        .any(|seg| !seg.is_empty() && util::contains_any(seg, CREDENTIAL_SEGMENTS))
}

/// fs_delete 专属红线：多路径批量删，或递归删用户目录。
fn check_fs_delete(call: &ToolCall) -> Option<Redline> {
    if let Some(paths) = call.param("paths") {
        if util::split_words(paths).len() >= 2 {
            return Some(Redline::BatchDelete);
        }
    }
    let recursive = call.param("recursive") == Some("true");
    if recursive {
        if let Some(p) = call.param("path") {
            if is_guard_delete_path(p) {
                return Some(Redline::BatchDelete);
            }
        }
    }
    None
}
pub use crate::redline_data::*;
