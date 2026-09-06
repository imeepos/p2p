//! 可执行文件定位（gui-contract.md §15 顺序）：ACP_CONSOLE_BIN → 应用可执行文件
//! 同目录 → 开发回退 cargo target 的 debug/release。三步均未命中返回 Err，
//! 由调用方转 unavailable 并显式留痕（不阻断 GUI 主功能）。

use std::path::{Path, PathBuf};

/// 按契约顺序定位 acp-console，命中首个存在的可执行文件。
pub(crate) fn locate() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位应用可执行文件失败: {e}"))?;
    locate_with(std::env::var("ACP_CONSOLE_BIN").ok().as_deref(), &exe)
}

/// 依赖注入形态（env 取值 + 应用可执行文件路径），单测覆盖三步顺序。
pub(crate) fn locate_with(env_bin: Option<&str>, exe: &Path) -> Result<PathBuf, String> {
    let mut tried: Vec<String> = Vec::new();
    if let Some(bin) = env_bin {
        let candidate = PathBuf::from(bin);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
        tried.push(format!("ACP_CONSOLE_BIN={}", candidate.display()));
    }
    if let Some(dir) = exe.parent() {
        let candidate = dir.join(binary_name());
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
        tried.push(candidate.display().to_string());
    }
    for candidate in dev_fallback_candidates(exe) {
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
        tried.push(candidate.display().to_string());
    }
    Err(format!(
        "acp-console 三步定位均未命中：{}",
        tried.join("；")
    ))
}

/// 开发回退：自可执行目录逐级向上（至多 6 级）查 <级>/target/{debug,release}/<bin>，
/// 覆盖 src-tauri 同级 target 与仓库根 workspace target 两种布局；debug 先于 release。
fn dev_fallback_candidates(exe: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dir = exe.parent();
    for _ in 0..6 {
        let Some(d) = dir else { break };
        for profile in ["debug", "release"] {
            out.push(d.join("target").join(profile).join(binary_name()));
        }
        dir = d.parent();
    }
    out
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "acp-console.exe"
    } else {
        "acp-console"
    }
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("ux_console_loc_{tag}_{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("创建临时目录");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            if let Err(e) = std::fs::remove_dir_all(&self.0) {
                eprintln!("[locate-test] 清理临时目录失败 {}: {e}", self.0.display());
            }
        }
    }

    fn touch_exec(path: &Path) {
        std::fs::create_dir_all(path.parent().expect("有父目录")).expect("建目录");
        std::fs::write(path, "#!/bin/sh\n").expect("写文件");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("加执行位");
    }

    fn dev_exe(root: &Path) -> PathBuf {
        root.join("apps/gui/src-tauri/target/debug/p2p-console")
    }

    #[test]
    fn env_var_wins_over_other_candidates() {
        let dir = TempDir::new("env");
        let env_bin = dir.0.join("env-bin/acp-console");
        touch_exec(&env_bin);
        let exe = dev_exe(&dir.0);
        touch_exec(&exe);
        let same_dir = exe.parent().unwrap().join(binary_name());
        touch_exec(&same_dir);
        let found = locate_with(env_bin.to_str(), &exe).expect("命中 env 候选");
        assert_eq!(found, env_bin, "环境变量候选必须最优先");
    }

    #[test]
    fn env_var_missing_file_falls_through() {
        let dir = TempDir::new("envmiss");
        let exe = dev_exe(&dir.0);
        touch_exec(&exe);
        let sidecar = exe.parent().unwrap().join(binary_name());
        touch_exec(&sidecar);
        let missing = dir.0.join("nope/acp-console");
        let found = locate_with(missing.to_str(), &exe).expect("回落同目录候选");
        assert_eq!(found, sidecar, "env 指向缺失文件时必须继续后续候选");
    }

    #[test]
    fn same_dir_as_app_executable_hits() {
        let dir = TempDir::new("samedir");
        let exe = dir.0.join("bundle/macos/p2p-console");
        touch_exec(&exe);
        let sidecar = exe.parent().unwrap().join(binary_name());
        touch_exec(&sidecar);
        let found = locate_with(None, &exe).expect("命中同目录候选");
        assert_eq!(found, sidecar);
    }

    #[test]
    fn dev_fallback_walks_up_to_workspace_target_debug() {
        let dir = TempDir::new("devdebug");
        let root_bin = dir.0.join("target/debug").join(binary_name());
        touch_exec(&root_bin);
        let exe = dev_exe(&dir.0);
        touch_exec(&exe);
        let found = locate_with(None, &exe).expect("命中仓库根 target/debug");
        assert_eq!(found, root_bin);
    }

    #[test]
    fn dev_fallback_accepts_release_profile() {
        let dir = TempDir::new("devrel");
        let root_bin = dir.0.join("target/release").join(binary_name());
        touch_exec(&root_bin);
        let exe = dev_exe(&dir.0);
        touch_exec(&exe);
        let found = locate_with(None, &exe).expect("命中 release 布局");
        assert_eq!(found, root_bin);
    }

    #[test]
    fn non_executable_file_is_skipped() {
        let dir = TempDir::new("noexec");
        let exe = dev_exe(&dir.0);
        touch_exec(&exe);
        let sidecar = exe.parent().unwrap().join(binary_name());
        std::fs::write(&sidecar, "plain data").expect("写无执行位文件");
        assert!(
            locate_with(None, &exe).is_err(),
            "无执行位的同名文件不得命中"
        );
    }

    #[test]
    fn nothing_found_reports_tried_candidates() {
        let dir = TempDir::new("none");
        let exe = dev_exe(&dir.0);
        touch_exec(&exe);
        let err = locate_with(None, &exe).expect_err("必须失败");
        assert!(err.contains("三步定位均未命中"), "失败原因须列候选: {err}");
    }
}
