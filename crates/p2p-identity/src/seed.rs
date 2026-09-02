//! 身份种子落盘与加载：文件权限 0600，路径由调用方传入（coordination.md K 包）。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use super::Keypair;

const SEED_LEN: usize = 32;

/// 文件存在则加载，不存在则生成新身份并落盘；加载失败原样上抛，不静默重建。
pub fn load_or_generate(path: &Path) -> io::Result<Keypair> {
    match fs::metadata(path) {
        Ok(_) => load(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let keypair = Keypair::generate();
            save(path, &keypair)?;
            Ok(keypair)
        }
        Err(e) => Err(e),
    }
}

/// 加载种子文件；非 0600 权限会被收紧，防止身份密钥泄露。
pub fn load(path: &Path) -> io::Result<Keypair> {
    enforce_private_mode(path)?;
    let bytes = fs::read(path)?;
    if bytes.len() != SEED_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("seed file must be {SEED_LEN} bytes, got {}", bytes.len()),
        ));
    }
    let mut seed = [0u8; SEED_LEN];
    seed.copy_from_slice(&bytes);
    Ok(Keypair::from_seed(&seed))
}

/// 落盘身份种子：以 0600 权限创建文件，父目录按需创建。
pub fn save(path: &Path, keypair: &Keypair) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&keypair.to_seed_bytes())?;
    file.flush()
}

#[cfg(unix)]
fn enforce_private_mode(path: &Path) -> io::Result<()> {
    let meta = fs::metadata(path)?;
    let mode = meta.permissions().mode();
    if mode & 0o077 != 0 {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn enforce_private_mode(_path: &Path) -> io::Result<()> {
    Ok(())
}
