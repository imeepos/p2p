//! 路径监狱：授权根列表 + 路径参数解析与越界拒绝。
//!
//! 语义（remote-support-plan.md §3.5）：全部 fs_* 路径参数经 canonicalize 后
//! 必须落在至少一个授权根内；「..」拼接、绝对路径越界、符号链接逃逸一律拒绝，
//! 拒绝带原因。相对路径基于首个授权根解析，对远程调用方语义确定。
//!
//! 实现要点：canonicalize 展开符号链接与归一路径，越界判定用组件级前缀
//! （[Path::starts_with]），杜绝 "/root-evil" 这类字符串前缀误判。

use std::path::{Path, PathBuf};

/// 授权根列表 + 越界判定。
#[derive(Debug, Clone)]
pub struct PathJail {
    roots: Vec<PathBuf>,
}

impl PathJail {
    /// 由授权根列表构造：每个根必须存在、是目录且可 canonicalize。
    pub fn from_roots(roots: Vec<PathBuf>) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("jail requires at least one authorized root".into());
        }
        let mut canon = Vec::with_capacity(roots.len());
        for root in roots {
            let resolved = std::fs::canonicalize(&root)
                .map_err(|e| format!("authorized root not accessible: {} ({e})", root.display()))?;
            if !resolved.is_dir() {
                return Err(format!(
                    "authorized root is not a directory: {}",
                    resolved.display()
                ));
            }
            canon.push(resolved);
        }
        Ok(Self { roots: canon })
    }

    /// 临时演示根（缺省配置）：temp_dir()/repair-helper-demo。
    pub fn demo() -> Result<Self, String> {
        let dir = std::env::temp_dir().join("repair-helper-demo");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("demo root create failed: {} ({e})", dir.display()))?;
        Self::from_roots(vec![dir])
    }

    /// 授权根列表（canonicalize 后）。
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// 首个授权根的克隆（缺省解析基准）。
    pub fn first_root(&self) -> Result<PathBuf, String> {
        self.roots
            .first()
            .cloned()
            .ok_or_else(|| "jail has no authorized roots".to_string())
    }

    /// 路径参数解析：非空 + 无「..」段 + canonicalize + 落在授权根内。
    /// 拒绝一律带原因。
    pub fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        if raw.trim().is_empty() {
            return Err("path is empty".into());
        }
        if has_dotdot(raw) {
            return Err(format!("path contains '..' escape attempt: {raw}"));
        }
        let base = self.first_root()?;
        let candidate = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            base.join(raw)
        };
        let resolved = std::fs::canonicalize(&candidate)
            .map_err(|e| format!("cannot resolve path '{raw}': {e}"))?;
        if !self.within(&resolved) {
            return Err(format!(
                "path escapes authorized roots: {}",
                resolved.display()
            ));
        }
        Ok(resolved)
    }

    /// 已解析路径是否落在任一授权根内（组件级前缀，含根自身）。
    pub fn within(&self, path: &Path) -> bool {
        self.roots
            .iter()
            .any(|root| path == root || path.starts_with(root))
    }
}

/// 原始路径是否含「..」路径段（按 '/' 与 '\\' 切分，跨平台归一）。
fn has_dotdot(raw: &str) -> bool {
    raw.split(['/', '\\']).any(|seg| seg == "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(tag: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("rh-jail-{}-{tag}", std::process::id()));
        let root = base.join("jail");
        let outside = base.join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(root.join("a.txt"), "hello").unwrap();
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        (root, outside)
    }

    #[test]
    fn relative_path_resolves_inside_root() {
        let (root, _) = fixture("rel");
        let jail = PathJail::from_roots(vec![root.clone()]).unwrap();
        let got = jail.resolve("a.txt").unwrap();
        let expect = std::fs::canonicalize(root.join("a.txt")).unwrap();
        assert_eq!(got, expect);
    }

    #[test]
    fn absolute_path_inside_root_allowed() {
        let (root, _) = fixture("abs");
        let jail = PathJail::from_roots(vec![root.clone()]).unwrap();
        let target = std::fs::canonicalize(root.join("a.txt")).unwrap();
        assert!(jail.resolve(target.to_str().unwrap()).is_ok());
    }

    #[test]
    fn dotdot_escape_rejected_with_reason() {
        let (root, _) = fixture("dotdot");
        let jail = PathJail::from_roots(vec![root]).unwrap();
        for bad in ["../outside/secret.txt", "sub/../secret.txt"] {
            let err = jail.resolve(bad).unwrap_err();
            assert!(err.contains("'..'"), "unexpected reason: {err}");
        }
    }

    #[test]
    fn absolute_escape_rejected_with_reason() {
        let (root, outside) = fixture("esc");
        let jail = PathJail::from_roots(vec![root]).unwrap();
        let err = jail.resolve(outside.to_str().unwrap()).unwrap_err();
        assert!(
            err.contains("escapes authorized roots"),
            "unexpected: {err}"
        );
        assert!(
            err.contains("outside"),
            "reason should name the resolved path: {err}"
        );
    }

    #[test]
    fn nonexistent_path_rejected() {
        let (root, _) = fixture("nope");
        let jail = PathJail::from_roots(vec![root]).unwrap();
        assert!(jail.resolve("does-not-exist.txt").is_err());
        assert!(jail.resolve("").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_rejected_with_reason() {
        use std::os::unix::fs::symlink;
        let (root, outside) = fixture("link");
        symlink(&outside, root.join("escape")).unwrap();
        let jail = PathJail::from_roots(vec![root]).unwrap();
        let err = jail.resolve("escape").unwrap_err();
        assert!(
            err.contains("escapes authorized roots"),
            "unexpected: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_inside_root_allowed() {
        use std::os::unix::fs::symlink;
        let (root, _) = fixture("inlink");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("real.txt"), "x").unwrap();
        symlink(root.join("real.txt"), root.join("alias.txt")).unwrap();
        let jail = PathJail::from_roots(vec![root.clone()]).unwrap();
        let got = jail.resolve("alias.txt").unwrap();
        assert_eq!(got, fs::canonicalize(root.join("real.txt")).unwrap());
    }

    #[test]
    fn demo_root_is_valid() {
        let jail = PathJail::demo().unwrap();
        let root = jail.first_root().unwrap();
        assert!(root.is_dir());
        assert!(jail.within(&root));
    }

    #[test]
    fn multi_root_any_match() {
        let (r1, r2) = fixture("multi");
        let jail = PathJail::from_roots(vec![r1.clone(), r2.clone()]).unwrap();
        assert!(jail
            .resolve(r2.join("secret.txt").to_str().unwrap())
            .is_ok());
    }

    #[test]
    fn empty_roots_rejected() {
        assert!(PathJail::from_roots(Vec::new()).is_err());
    }
}
