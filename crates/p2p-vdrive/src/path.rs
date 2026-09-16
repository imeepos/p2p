//! 虚拟路径规范化（vdrive 两端共用的路径契约）。

/// 虚拟路径规范化：绝对化 + 折叠 `.`/空段/`..`；越出根返回 None。
pub fn normalize_vpath(vpath: &str) -> Option<String> {
    let vpath = vpath.strip_suffix('/').unwrap_or(vpath);
    let mut parts: Vec<&str> = Vec::new();
    for seg in vpath.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_folds_and_rejects_escape() {
        assert_eq!(normalize_vpath("/a/b"), Some("/a/b".into()));
        assert_eq!(normalize_vpath("a//b/./c"), Some("/a/b/c".into()));
        assert_eq!(normalize_vpath("/a/b/.."), Some("/a".into()));
        assert_eq!(normalize_vpath("/a/.."), Some("/".into()));
        assert_eq!(normalize_vpath("/.."), None);
        assert_eq!(normalize_vpath("/a/../../x"), None);
        assert_eq!(normalize_vpath("/dir/"), Some("/dir".into()));
    }
}
