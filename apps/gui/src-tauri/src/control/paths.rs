//! 控制通道数据文件：token（600 权限）与端点状态文件，均落 GUI 数据目录 control/ 下。
//! CLI（GC2）靠这两个文件发现通道地址与凭证。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// 端点状态文件内容（endpoint.json）；字段 camelCase 与仓库契约惯例一致。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointInfo {
    pub http: String,
    pub pid: u32,
    pub version: String,
    pub started_at_ms: u64,
    pub token_file: String,
}

pub fn control_dir(base: &Path) -> PathBuf {
    base.join("control")
}

/// 读已有 token；缺失或异常时重新生成（异常重建留 warn 可观测）。
pub fn load_or_create_token(dir: &Path) -> Result<String, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("创建控制通道目录 {} 失败: {e}", dir.display()))?;
    let file = dir.join("token");
    if let Ok(raw) = std::fs::read_to_string(&file) {
        let token = raw.trim();
        if token.len() >= 32 && token.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Ok(token.to_string());
        }
        tracing::warn!("control: token 文件内容异常（过短或非 hex），重新生成");
    }
    let token = generate_token();
    write_token_0600(&file, &token)?;
    Ok(token)
}

/// 32 字节 OS 随机源 → 64 位 hex 字符串。
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("OS 随机源不可用（getrandom 失败）");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 建文件即 0600；已存在文件权限纠偏（unix）。
fn write_token_0600(file: &Path, token: &str) -> Result<(), String> {
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(file)
    };
    #[cfg(not(unix))]
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file);
    let mut f = created.map_err(|e| format!("创建 token 文件 {} 失败: {e}", file.display()))?;
    f.write_all(token.as_bytes())
        .map_err(|e| format!("写 token 失败: {e}"))?;
    f.sync_all().ok();
    #[cfg(unix)]
    if let Ok(meta) = std::fs::metadata(file) {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = meta.permissions();
        if perm.mode() & 0o777 != 0o600 {
            tracing::warn!("control: token 文件权限非 600，纠偏");
            perm.set_mode(0o600);
            if let Err(e) = std::fs::set_permissions(file, perm) {
                tracing::error!("control: token 权限纠偏失败: {e}");
            }
        }
    }
    Ok(())
}

/// 端点状态文件：成功绑定后写、退出时摘；失败留错误日志（可观测）。
pub fn write_endpoint(dir: &Path, info: &EndpointInfo) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
    let file = dir.join("endpoint.json");
    let body = serde_json::to_vec_pretty(info).map_err(|e| format!("endpoint 序列化失败: {e}"))?;
    std::fs::write(&file, body).map_err(|e| format!("写 endpoint.json 失败: {e}"))
}

/// 摘端点文件；NotFound 视为已摘，其余失败留 warn。
pub fn remove_endpoint(dir: &Path) {
    match std::fs::remove_file(dir.join("endpoint.json")) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!("control: 清理 endpoint.json 失败: {e}"),
    }
}

/// 恒时比较，避免 token 逐字节短路泄漏时序信息。
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
