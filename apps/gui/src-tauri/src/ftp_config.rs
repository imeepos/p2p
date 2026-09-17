//! FTP 服务端配置命令面（W2b/CC4）：gui-contract §3.1 本机服务配置面。
//!
//! 数据根 = app 数据目录（services.json/authz §18 同口径）；serde 镜像字段名
//! 与 apps/cli FtpDaemonConfig 逐字一致（root/accounts/authz；bin crate 不可
//! 依赖，独立定义）。文件缺失/损坏回空值 + warn 不静默（daemon ftp_serve
//! fail-safe 跳过装配同语义）；get 仅回用户名不回显密码；save 整表替换、
//! 空密码 = 保留现有密码、原子写（tmp+rename）且 0600。效果语义：CLI daemon
//! 启动装配读取（serve.ftp AND ftp.json），改后重启生效，无 live reload。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::warn;

use crate::state::AppState;

pub(crate) const FILE_NAME: &str = "ftp.json";

/// ftp.json serde 镜像（字段名与 apps/cli FtpDaemonConfig 逐字一致）。
#[derive(Debug, Default, Deserialize, Serialize)]
struct FtpConfigFile {
    root: String,
    #[serde(default)]
    accounts: HashMap<String, String>,
    #[serde(default)]
    authz: bool,
}

/// ftp_config_get 回包：仅用户名，密码不回显（契约逐字）。
#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpConfigView {
    pub root: String,
    pub authz: bool,
    pub users: Vec<String>,
}

/// ftp_config_get：文件缺失/损坏回空值 + warn（契约逐字，禁静默）。
/// tauri 约束（async + State 入参须 Result）：恒 Ok，IPC resolve 值即契约形状。
#[tauri::command]
pub async fn ftp_config_get(state: State<'_, AppState>) -> Result<FtpConfigView, String> {
    Ok(load_view(&config_path(state.data_dir())))
}

/// ftp_config_save：整表替换；空密码 = 保留该用户现有密码；原子写 0600。
#[tauri::command]
pub async fn ftp_config_save(
    state: State<'_, AppState>,
    root: String,
    authz: bool,
    accounts: HashMap<String, String>,
) -> Result<bool, String> {
    save_config(&config_path(state.data_dir()), &root, authz, &accounts)?;
    Ok(true)
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

/// 读 + 投影为视图（用户名排序，输出确定性）。
fn load_view(path: &Path) -> FtpConfigView {
    match load_file(path) {
        Ok(cfg) => FtpConfigView {
            root: cfg.root,
            authz: cfg.authz,
            users: sorted_users(&cfg.accounts),
        },
        Err(_) => FtpConfigView::default(),
    }
}

/// 读文件：缺失 = 空配置（Ok）；损坏/不可读 = Err 并 warn（get 侧据此回空值）。
fn load_file(path: &Path) -> Result<FtpConfigFile, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            warn!(path = %path.display(), "ftp.json 不存在，按空配置处理");
            return Ok(FtpConfigFile::default());
        }
        Err(e) => {
            warn!(error = %e, path = %path.display(), "ftp.json 读取失败，按空配置处理");
            return Err(format!("读取 ftp.json 失败: {e}"));
        }
    };
    match serde_json::from_str(&text) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            warn!(error = %e, path = %path.display(), "ftp.json 解析失败，按空配置处理");
            Err(format!("ftp.json 解析失败: {e}"))
        }
    }
}

/// 密码合并：空密码 = 保留现有密码（契约）；新账号空密码显式拒绝
/// （凭据面 fail-safe：空密码账号会被 StaticAuth 空串匹配放行）。
fn merge_accounts(
    existing: &HashMap<String, String>,
    incoming: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let mut merged = HashMap::new();
    for (user, pass) in incoming {
        if pass.is_empty() {
            match existing.get(user) {
                Some(saved) => {
                    merged.insert(user.clone(), saved.clone());
                }
                None => return Err(format!("账号 {user} 为新账号，密码不能为空")),
            }
        } else {
            merged.insert(user.clone(), pass.clone());
        }
    }
    Ok(merged)
}

/// 保存内核（可测）：重读磁盘合并密码 → 原子写 0600；损坏文件显式拒存
/// （不静默覆盖，防误清账号表）。
fn save_config(
    path: &Path,
    root: &str,
    authz: bool,
    accounts: &HashMap<String, String>,
) -> Result<(), String> {
    let existing = load_file(path).map_err(|e| format!("保存拒绝（{e}）"))?;
    let merged = merge_accounts(&existing.accounts, accounts)?;
    let cfg = FtpConfigFile {
        root: root.to_string(),
        accounts: merged,
        authz,
    };
    let text =
        serde_json::to_string_pretty(&cfg).map_err(|e| format!("ftp.json 序列化失败: {e}"))?;
    write_atomic_0600(path, text.as_bytes())
}

/// 原子写（tmp+rename）且 0600：tmp 以 0600 建立再 rename，无明文宽松窗口。
fn write_atomic_0600(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    #[cfg(unix)]
    let wrote = (|| {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(bytes)
    })();
    #[cfg(not(unix))]
    let wrote = fs::write(&tmp, bytes);
    if let Err(e) = wrote {
        let _ = fs::remove_file(&tmp);
        warn!(error = %e, path = %tmp.display(), "写入 ftp.json 临时文件失败");
        return Err(format!("保存 ftp.json 失败: {e}"));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        warn!(error = %e, path = %path.display(), "替换 ftp.json 失败");
        format!("保存 ftp.json 失败: {e}")
    })
}

fn sorted_users(accounts: &HashMap<String, String>) -> Vec<String> {
    let mut users: Vec<String> = accounts.keys().cloned().collect();
    users.sort();
    users
}

#[cfg(test)]
mod tests;
