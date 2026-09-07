//! provider 配置持久化（llm-share-link 设计 §5.3 B 段，W2）：providers.json
//! （v1 信封，只存 id/name/baseUrl/protocol/models/createdAt + apiKeyRef，不存明文 key）
//! + 独立 0600 密钥文件 keys/<providerId>.key；写路径原子落盘、目录收紧 0700。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::allowlist::normalize_models;
use super::file_path;
use super::read_json_or_none;
use super::write_json_atomic;

pub const FILE_NAME: &str = "providers.json";
pub const FORMAT_VERSION: u8 = 1;
pub const KEYS_DIR: &str = "keys";

/// 上游协议（出借方按协议选上游实现；借方恒 OpenAI 兼容，差异由出借方翻译）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    OpenAI,
    Claude,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::OpenAI => "openai",
            Protocol::Claude => "claude",
        }
    }
}

/// provider 配置（密钥不在此结构，落独立 0600 文件）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    pub models: Vec<String>,
    pub created_at: u64,
}

/// providers.json 条目：config 扁平 + apiKeyRef。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProvider {
    #[serde(flatten)]
    pub config: ProviderConfig,
    pub api_key_ref: String,
}

/// 落盘信封（版本 + 条目表；BTreeMap 键序稳定可复现）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFile {
    pub v: u8,
    pub providers: BTreeMap<String, StoredProvider>,
}

impl ProviderFile {
    pub fn new() -> Self {
        Self {
            v: FORMAT_VERSION,
            providers: BTreeMap::new(),
        }
    }
}

pub fn path(data_dir: &str) -> PathBuf {
    file_path(data_dir, FILE_NAME)
}

/// 密钥文件路径：<data-dir>/llm-share/keys/<id>.key。
pub fn key_path(data_dir: &str, id: &str) -> PathBuf {
    file_path(data_dir, KEYS_DIR).join(format!("{id}.key"))
}

/// 读 provider 存档：缺失=空表；损坏/版本不符显式报错。
pub fn load_or_empty(path: &Path) -> Result<ProviderFile, String> {
    let parsed: Option<ProviderFile> = read_json_or_none(path, "provider 存档")?;
    match parsed {
        Some(file) => {
            if file.v != FORMAT_VERSION {
                return Err(format!(
                    "provider 存档版本不支持（v{}，当前 v{FORMAT_VERSION}）：{}",
                    file.v,
                    path.display()
                ));
            }
            Ok(file)
        }
        None => Ok(ProviderFile::new()),
    }
}

/// apiKey 掩码：≤8 位全掩 "****"，否则 前4+"****"+后4。
pub fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_owned()
    } else {
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    }
}

/// provider 保存入参（apps/cli clap 层装配，本层校验）。
pub struct SaveParams {
    /// 缺省生成 UUID。
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    pub api_key: String,
    pub models: Vec<String>,
    pub created_at: u64,
}

/// 保存 provider：必填校验 → 模型唯一映射校验 → 先落 0600 密钥 → 再原子写存档。
pub fn save(data_dir: &str, params: SaveParams) -> Result<ProviderConfig, String> {
    let name = params.name.trim().to_owned();
    let base_url = params.base_url.trim().to_owned();
    let api_key = params.api_key.trim().to_owned();
    if name.is_empty() {
        return Err("provider 名称不能为空".to_owned());
    }
    if base_url.is_empty() {
        return Err("provider baseUrl 不能为空".to_owned());
    }
    if api_key.is_empty() {
        return Err("apiKey 不能为空（--api-key 或 stdin 提供）".to_owned());
    }
    let models = normalize_models(&params.models)?;
    if models.is_empty() {
        return Err("provider 模型不能为空（--model 至少一个）".to_owned());
    }
    let id = params
        .id
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let file = path(data_dir);
    let mut store = load_or_empty(&file)?;
    for (other_id, stored) in &store.providers {
        if *other_id == id {
            continue;
        }
        for model in &stored.config.models {
            if models.contains(model) {
                return Err(format!(
                    "模型 {model} 已被 provider {}（{other_id}）占用：模型→provider 唯一映射",
                    stored.config.name
                ));
            }
        }
    }
    let key_file = key_path(data_dir, &id);
    write_key_file(&key_file, &api_key)?;
    let config = ProviderConfig {
        id: id.clone(),
        name,
        base_url,
        protocol: params.protocol,
        models: models.clone(),
        created_at: params.created_at,
    };
    let api_key_ref = format!("{id}.key");
    store.providers.insert(
        id,
        StoredProvider {
            config: config.clone(),
            api_key_ref,
        },
    );
    write_json_atomic(&file, &store, "provider 存档")?;
    Ok(config)
}

/// provider 视图（apiKey 只出掩码）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    pub models: Vec<String>,
    pub created_at: u64,
    pub api_key_masked: String,
}

/// 列表：密钥文件缺失=显式报错（不静默回退掩码空串）。
pub fn list(data_dir: &str) -> Result<Vec<ProviderView>, String> {
    let store = load_or_empty(&path(data_dir))?;
    let mut views = Vec::with_capacity(store.providers.len());
    for (id, stored) in store.providers {
        let key_file = key_path(data_dir, &id);
        let key = std::fs::read_to_string(&key_file).map_err(|e| {
            format!(
                "provider {} 密钥文件缺失/不可读（{}）: {e}",
                stored.config.name,
                key_file.display()
            )
        })?;
        views.push(ProviderView {
            id,
            name: stored.config.name,
            base_url: stored.config.base_url,
            protocol: stored.config.protocol,
            models: stored.config.models,
            created_at: stored.config.created_at,
            api_key_masked: mask_key(key.trim()),
        });
    }
    Ok(views)
}

/// 单查（share_create 装配用）：不存在返回 None。
pub fn get(data_dir: &str, id: &str) -> Result<Option<ProviderConfig>, String> {
    let store = load_or_empty(&path(data_dir))?;
    Ok(store.providers.get(id).map(|stored| stored.config.clone()))
}

/// 移除 provider：不存在=显式报错；存在则移除 + 原子写 + 级联删密钥文件。
pub fn remove(data_dir: &str, id: &str) -> Result<bool, String> {
    let file = path(data_dir);
    let mut store = load_or_empty(&file)?;
    if !store.providers.contains_key(id) {
        return Err(format!("provider 不存在：{id}"));
    }
    store.providers.remove(id);
    write_json_atomic(&file, &store, "provider 存档")?;
    let key_file = key_path(data_dir, id);
    if key_file.exists() {
        std::fs::remove_file(&key_file)
            .map_err(|e| format!("密钥文件删除失败（{}）: {e}", key_file.display()))?;
    }
    Ok(true)
}

/// 0600 密钥原子落盘（tmp+rename + 显式 chmod）；keys 与 llm-share 目录收紧 0700。
fn write_key_file(path: &Path, key: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("密钥目录创建失败（{}）: {e}", dir.display()))?;
        ensure_private_dir(dir)?;
        if let Some(llm_share_dir) = dir.parent() {
            ensure_private_dir(llm_share_dir)?;
        }
    }
    let tmp = path.with_extension("key.tmp");
    std::fs::write(&tmp, key.as_bytes())
        .map_err(|e| format!("密钥写入失败（{}）: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("密钥权限设置失败（{}）: {e}", tmp.display()))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("密钥保存失败（{}）: {e}", path.display())
    })
}

/// 目录权限收紧 0700（非 unix 平台 no-op，沿用用户目录 ACL）。
#[cfg(unix)]
fn ensure_private_dir(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("目录权限设置失败（{}）: {e}", dir.display()))
}

#[cfg(not(unix))]
fn ensure_private_dir(_dir: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests;
