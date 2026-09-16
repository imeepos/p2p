//! DAV 变更面辅助：递归删除/复制、Destination 解析、父路径计算。

use std::sync::Arc;

use crate::backend::FsBackend;
use crate::error::{ErrorKind, VDriveError};
use crate::http::percent_decode;
use crate::wire::EntryKind;

/// 父虚拟路径：末段剥离；根/一层路径的父为 `/`。
pub fn parent_vpath(vpath: &str) -> Option<String> {
    let trimmed = vpath.trim_end_matches('/');
    match trimmed.rfind('/') {
        None => None,
        Some(0) => Some("/".into()),
        Some(idx) => Some(trimmed[..idx].to_string()),
    }
}

/// Destination 头 → 虚拟路径：接受绝对 URI 或绝对路径；percent 解码。
pub fn destination_vpath(header: &str) -> Option<String> {
    let raw = if let Some(rest) = header
        .strip_prefix("http://")
        .or_else(|| header.strip_prefix("https://"))
    {
        match rest.find('/') {
            Some(idx) => &rest[idx..],
            None => "/",
        }
    } else {
        header
    };
    let decoded = percent_decode(raw.split('?').next().unwrap_or(raw));
    crate::localfs::normalize_vpath(&decoded)
}

/// 递归删除（Depth: infinity 语义）。根路径由调用方显式拒绝。
pub fn delete_tree<'a>(
    backend: &'a Arc<dyn FsBackend>,
    vpath: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), VDriveError>> + Send + 'a>> {
    Box::pin(async move {
        let entry = backend.stat(vpath).await?;
        match entry.kind {
            EntryKind::File => backend.unlink(vpath).await,
            EntryKind::Dir => {
                for child in backend.list(vpath).await? {
                    let child_path = join_vpath(vpath, &child.name);
                    delete_tree(backend, &child_path).await?;
                }
                backend.rmdir(vpath).await
            }
        }
    })
}

/// 递归复制（目录深拷贝；文件按 chunk 定位读写）。
pub fn copy_tree<'a>(
    backend: &'a Arc<dyn FsBackend>,
    from: &'a str,
    to: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), VDriveError>> + Send + 'a>> {
    Box::pin(async move {
        let entry = backend.stat(from).await?;
        match entry.kind {
            EntryKind::File => copy_file(backend, from, to, entry.size).await,
            EntryKind::Dir => {
                backend.mkdir(to).await?;
                for child in backend.list(from).await? {
                    copy_tree(
                        backend,
                        &join_vpath(from, &child.name),
                        &join_vpath(to, &child.name),
                    )
                    .await?;
                }
                Ok(())
            }
        }
    })
}

async fn copy_file(
    backend: &Arc<dyn FsBackend>,
    from: &str,
    to: &str,
    size: u64,
) -> Result<(), VDriveError> {
    backend.create(to).await?;
    let mut offset = 0u64;
    while offset < size {
        let want = ((size - offset).min(crate::wire::MAX_CHUNK as u64)) as u32;
        let bytes = backend.read(from, offset, want).await?;
        if bytes.is_empty() {
            break;
        }
        backend.write(to, offset, &bytes).await?;
        offset += bytes.len() as u64;
    }
    Ok(())
}

pub fn join_vpath(base: &str, name: &str) -> String {
    let base = base.trim_end_matches('/');
    format!("{base}/{name}")
}

/// 目标父目录必须存在且为目录（409 语义的数据面）。
pub async fn dest_parent_ok(backend: &Arc<dyn FsBackend>, dest: &str) -> bool {
    match parent_vpath(dest) {
        None => false,
        Some(parent) => matches!(
            backend.stat(&parent).await,
            Ok(e) if e.kind == EntryKind::Dir
        ),
    }
}

/// MOVE 前的目标腾位：overwrite=false 且目标存在 → Err(412 数据)；
/// overwrite=true 且目标存在 → 递归删除。
pub async fn clear_dest(
    backend: &Arc<dyn FsBackend>,
    dest: &str,
    overwrite: bool,
) -> Result<bool, VDriveError> {
    match backend.stat(dest).await {
        Ok(_) if !overwrite => Err(VDriveError::new(
            ErrorKind::AlreadyExists,
            "destination exists and overwrite is false",
        )),
        Ok(_) => {
            delete_tree(backend, dest).await?;
            Ok(true)
        }
        Err(e) if e.kind == ErrorKind::NotFound => {
            if !dest_parent_ok(backend, dest).await {
                return Err(VDriveError::new(
                    ErrorKind::NotFound,
                    "destination parent missing",
                ));
            }
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_paths() {
        assert_eq!(parent_vpath("/a"), Some("/".into()));
        assert_eq!(parent_vpath("/a/b"), Some("/a".into()));
        assert_eq!(parent_vpath("/a/b/"), Some("/a".into()));
        assert_eq!(parent_vpath("/"), None);
    }

    #[test]
    fn destination_forms() {
        assert_eq!(
            destination_vpath("http://127.0.0.1:8080/x%20y/z"),
            Some("/x y/z".into())
        );
        assert_eq!(destination_vpath("/plain/path"), Some("/plain/path".into()));
        assert_eq!(destination_vpath("http://host:1"), Some("/".into()));
        assert_eq!(destination_vpath("/a/../b"), Some("/b".into()));
    }
}
