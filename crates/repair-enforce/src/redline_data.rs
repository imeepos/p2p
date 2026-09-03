//! 红线判定关键词数据表（与 [crate::redline] 拆分以控制文件行数）。
//! 全部为静态数据，判定逻辑只消费这些表，新增关键词只改本文件。

/// format / 低级磁盘操作关键词。
pub static FORMAT_KEYWORDS: &[&str] = &[
    "format",
    "mkfs",
    "mke2fs",
    "fdisk",
    "sfdisk",
    "sgdisk",
    "gdisk",
    "parted",
    "wipefs",
    "shred",
    "dd",
    "diskpart",
    "low level",
    "lowlevel",
];

/// 加密用户文件关键词（ransomware 式行为，细到子命令级避免误伤
/// gpg --verify / openssl dgst 等只读用法）。
pub static ENCRYPT_KEYWORDS: &[&str] = &[
    "openssl enc",
    "gpg --encrypt",
    "gpg2 --encrypt",
    "gpg -c",
    "gpg --symmetric",
    "age --encrypt",
    "cryptsetup",
    "luksformat",
    "bitlocker",
    "manage-bde",
    "cipher /e",
    "veracrypt",
];

/// 杀毒软件产品/组件名。
pub static ANTIVIRUS_NAMES: &[&str] = &[
    "defender",
    "windefend",
    "mppreference",
    "realtimemonitoring",
    "msmpsvc",
    "msmpeng",
    "mcafee",
    "norton",
    "symantec",
    "avast",
    "avg",
    "kaspersky",
    "bitdefender",
    "eset",
    "avira",
    "malwarebytes",
    "antivirus",
];

/// 使杀毒失效的动作词（必须与产品名同现才判红线）。
/// 包含全小写粘连参数名（PowerShell 参数大小写不敏感，"disablerealtime
/// monitoring" 大小写任意都生效），匹配器会按拆词/拼接归一处理。
pub static ANTIVIRUS_ACTIONS: &[&str] = &[
    "stop",
    "disable",
    "uninstall",
    "delete",
    "kill",
    "remove",
    "taskkill",
    "turn off",
    "exclude",
    "bypass",
    "stop-service",
    "disablerealtimemonitoring",
    "disablerealtimeprotection",
];

/// 凭据文件名/目录段级模式（按路径段词边界匹配）。
pub static CREDENTIAL_SEGMENTS: &[&str] = &[
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "id_dsa",
    "secring",
    "keychain",
    "credentials",
    "credential",
    "password",
    "passwords",
    "passwd",
    "shadow",
    "secrets",
    "secret",
    "kdbx",
    "kdb",
    "wallet",
    "logins",
    "ssh",
    "private",
    "env",
    "privatekey",
    "keyring",
];

/// 凭据多词组（整串匹配）。
pub static CREDENTIAL_MULTIWORD: &[&str] = &[
    "login data",
    "master password",
    "password store",
    "credential manager",
];

/// 删除类命令首词（argv[0] 白名单判定的一环）。
pub static DELETE_COMMANDS: &[&str] = &["rm", "del", "erase", "rd", "rmdir", "remove-item"];

/// 递归删除标志。
pub static RECURSIVE_FLAGS: &[&str] = &["-r", "-rf", "-recursive", "--recursive", "-recurse", "/s"];

/// 用户目录保护区顶层目录名。
pub static USER_TOP_DIRS: &[&str] = &[
    "documents",
    "desktop",
    "downloads",
    "pictures",
    "music",
    "videos",
];
