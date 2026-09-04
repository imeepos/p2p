//! profile 命令域：对齐 GUI profile_get/profile_save（契约 v6 §11）。
//! 校验规则与 GUI 同源：name ≤64 字符、description ≤280、avatar data URL 白名单 + base64。

use clap::{Args, Subcommand};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::store;
use crate::types::NodeProfile;

pub const NAME_MAX_CHARS: usize = 64;
pub const DESCRIPTION_MAX_CHARS: usize = 280;
pub const AVATAR_MAX_LEN: usize = 200_000;

const AVATAR_MIME_PREFIXES: [&str; 3] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/webp;base64,",
];

#[derive(Subcommand)]
pub enum ProfileCommand {
    /// 读取节点资料（无文件输出默认值）
    Get(DirArgs),
    /// 保存节点资料 JSON（参数为 "-" 或省略时读 stdin）
    Save(SaveArgs),
}

#[derive(Args)]
pub struct DirArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct SaveArgs {
    /// 完整 NodeProfile JSON；"-" 或省略 = 读 stdin
    profile: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

pub async fn run(cmd: ProfileCommand) -> CliResult<()> {
    match cmd {
        ProfileCommand::Get(a) => get(a),
        ProfileCommand::Save(a) => save(a),
    }
}

fn get(args: DirArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let profile = store::load_profile(&paths);
    output::emit(args.json, &profile, &render(&profile))
}

fn save(args: SaveArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let text = match args.profile.as_deref() {
        Some("-") | None => read_stdin()?,
        Some(text) => text.to_string(),
    };
    if text.trim().is_empty() {
        return Err(CliError::Runtime(
            "资料内容为空：传入 NodeProfile JSON 或经 stdin 管道".into(),
        ));
    }
    let profile: NodeProfile = serde_json::from_str(text.trim())
        .map_err(|e| CliError::Runtime(format!("资料 JSON 解析失败: {e}")))?;
    validate(&profile).map_err(CliError::Runtime)?;
    store::save_profile(&paths, &profile)?;
    output::emit(args.json, &profile, &render(&profile))
}

fn read_stdin() -> CliResult<String> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| CliError::Runtime(format!("读取 stdin 失败: {e}")))?;
    Ok(buf)
}

/// 契约 §11 校验；Err 一律可读中文（与 GUI NodeProfile::validate 同规则）。
fn validate(profile: &NodeProfile) -> Result<(), String> {
    if profile.name.trim().chars().count() > NAME_MAX_CHARS {
        return Err(format!("节点名称过长，上限 {NAME_MAX_CHARS} 字符"));
    }
    if profile.description.chars().count() > DESCRIPTION_MAX_CHARS {
        return Err(format!("节点描述过长，上限 {DESCRIPTION_MAX_CHARS} 字符"));
    }
    if let Some(avatar) = &profile.avatar {
        validate_avatar(avatar)?;
    }
    Ok(())
}

fn validate_avatar(url: &str) -> Result<(), String> {
    if url.len() > AVATAR_MAX_LEN {
        return Err(format!("头像数据过大，上限 {AVATAR_MAX_LEN} 字符"));
    }
    let payload = AVATAR_MIME_PREFIXES
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))
        .ok_or_else(|| "头像格式不支持，仅允许 PNG/JPEG/WebP 的 base64 data URL".to_string())?;
    if !payload
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
    {
        return Err("头像数据不是合法的 base64 载荷".into());
    }
    Ok(())
}

fn render(profile: &NodeProfile) -> String {
    let avatar = match &profile.avatar {
        Some(url) => format!("avatar=已设置（{} 字符）", url.len()),
        None => "avatar=未设置".into(),
    };
    format!(
        "name={}\ndescription={}\n{avatar}",
        profile.name, profile.description
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_boundary_values() {
        let profile = NodeProfile {
            name: "n".repeat(NAME_MAX_CHARS),
            description: "d".repeat(DESCRIPTION_MAX_CHARS),
            avatar: Some(format!(
                "data:image/webp;base64,{}",
                "a".repeat(AVATAR_MAX_LEN - "data:image/webp;base64,".len(),)
            )),
        };
        assert_eq!(validate(&profile), Ok(()));
    }

    #[test]
    fn rejects_oversize_and_bad_avatar() {
        let mut profile = NodeProfile {
            name: "名".repeat(NAME_MAX_CHARS + 1),
            ..Default::default()
        };
        assert!(validate(&profile).is_err());
        profile.name = "ok".into();
        profile.description = "述".repeat(DESCRIPTION_MAX_CHARS + 1);
        assert!(validate(&profile).is_err());
        profile.description = String::new();
        profile.avatar = Some("data:image/gif;base64,aGk=".into());
        assert!(validate(&profile).is_err(), "MIME 白名单外拒绝");
        profile.avatar = Some("data:image/png;base64,###".into());
        assert!(validate(&profile).is_err(), "非法 base64 拒绝");
    }
}
