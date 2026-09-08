//! 分享 token：128-bit CSPRNG hex（32 位小写）+ sha256 摘要。
//! token 原文只在创建响应与链接中出现一次，台账只落摘要（设计 §5.4 C 段）。

use rand::RngCore;

/// 生成 32 位小写 hex token（16 字节 CSPRNG）。
pub fn generate_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// token 的 sha256 小写 hex 摘要（台账主索引）。
pub fn token_sha256(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::is_valid_token;

    #[test]
    fn generated_token_is_32_lowercase_hex() {
        let token = generate_token();
        assert!(is_valid_token(&token), "token: {token}");
    }

    #[test]
    fn generated_tokens_differ() {
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn sha256_is_64_hex_and_deterministic() {
        let digest = token_sha256("a");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        assert_eq!(token_sha256("a"), digest);
        assert_ne!(token_sha256("a"), token_sha256("b"));
    }
}
