//! 平台公钥解析（票据验签参数）：hex 64 字符（可带 0x 前缀/空白）→ ed25519
//! 32 字节；非法输入显式报错（serve --platform-pubkey 与 mint 前置共用）。

/// hex 64 字符 -> ed25519 公钥 32 字节；长度/非 hex 显式报错。
pub fn parse_pubkey_hex(s: &str) -> std::io::Result<[u8; 32]> {
    let compact: Vec<u8> = s
        .strip_prefix("0x")
        .unwrap_or(s)
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if compact.len() != 64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "platform public key must be 64 hex chars",
        ));
    }
    let mut out = [0u8; 32];
    for (i, pair) in compact.chunks(2).enumerate() {
        let hi = hex_val(pair[0]).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "public key not hex")
        })?;
        let lo = hex_val(pair[1]).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "public key not hex")
        })?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex64_with_prefix_and_whitespace() {
        let ok = parse_pubkey_hex(&format!("0x{} ", "ea4a".repeat(16))).unwrap();
        assert_eq!(ok[0], 0xea);
        assert_eq!(ok[31], 0x4a);
        let bad = parse_pubkey_hex("zz").unwrap_err();
        assert_eq!(bad.kind(), std::io::ErrorKind::InvalidInput);
        let short = parse_pubkey_hex("ea4a").unwrap_err();
        assert_eq!(short.kind(), std::io::ErrorKind::InvalidInput);
    }
}
