//! 上游 key 落盘（§6）：常态仅存出借方进程内存；落盘必须 0600，
//! 对齐 p2p-identity 种子标准；权限过松的历史文件在读取时收紧。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// 以 0600 写入 key（unix），父目录按需创建。
pub fn save(path: &Path, key: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    // 非 unix 沿用用户目录默认 ACL（目录已按用户隔离），与 p2p-identity 同口径。
    #[cfg(unix)]
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(key.as_bytes())?;
    file.flush()
}

/// 读取 key；权限含组/其他位时收紧为 0600 后再返回。
pub fn load(path: &Path) -> io::Result<String> {
    #[cfg(unix)]
    {
        let meta = fs::metadata(path)?;
        if meta.permissions().mode() & 0o077 != 0 {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)?;
        }
    }
    Ok(fs::read_to_string(path)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_key_roundtrips_with_private_mode() {
        let dir = std::env::temp_dir().join(format!("llm-proxy-key-{}", std::process::id()));
        let path = dir.join("gpt-4o.key");
        save(&path, "sk-test").expect("save");
        #[cfg(unix)]
        {
            let mode = fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_eq!(load(&path).expect("load"), "sk-test");
        fs::remove_file(&path).expect("cleanup");
        fs::remove_dir(&dir).expect("cleanup");
    }
}
