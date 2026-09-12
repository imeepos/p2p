//! RFC6455 WS 帧编解码与升级 accept 助手（仅测试夹具）。SHA-1/base64 为
//! 自足实现（RFC 3174 / RFC 4648 标准字母表）——只服务 RFC6455 accept 这一
//! 非安全哈希用途，零新增外部依赖（锁文件零改动）；正确性由 RFC 标准测试
//! 向量锚定（见文件尾 tests）。

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// RFC6455 升级 GUID（accept 计算固定后缀）。
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// RFC6455 accept：base64(sha1(key + GUID))。
pub fn ws_accept(key: &str) -> String {
    let mut payload = key.as_bytes().to_vec();
    payload.extend_from_slice(WS_GUID.as_bytes());
    b64(&sha1(&payload))
}

/// SHA-1（RFC 3174）：仅用于 RFC6455 accept 的非安全哈希。
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = [
        0x6745_2301u32,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        for (hi, round) in h.iter_mut().zip([a, b, c, d, e]) {
            *hi = hi.wrapping_add(round);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// 标准字母表 base64（含 '=' 填充；accept 输入恒 20 字节 → 输出 28 字符）。
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = (chunk[0] as u32) << 16;
        let b1 = chunk.get(1).map_or(0, |v| (*v as u32) << 8);
        let b2 = chunk.get(2).map_or(0, |v| *v as u32);
        let n = b0 | b1 | b2;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// 发一帧（客户端侧按 RFC6455 加掩码；掩码值固定即可，不进断言）。
pub async fn ws_send_frame(
    sock: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
) -> std::io::Result<()> {
    let mask = [0x2a, 0x5f, 0x11, 0x7c];
    let mut frame = vec![0x80 | opcode];
    match payload.len() {
        n @ 0..=125 => frame.push(0x80 | n as u8),
        n @ 126..=65535 => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().zip(mask.iter().cycle()).map(|(b, m)| b ^ m));
    sock.write_all(&frame).await
}

/// 收一帧（服务端帧无掩码；客户端帧解掩码），返回 (opcode, payload)。
pub async fn ws_recv_frame(
    sock: &mut (impl tokio::io::AsyncRead + Unpin),
) -> std::io::Result<(u8, Vec<u8>)> {
    let mut hdr = [0u8; 2];
    sock.read_exact(&mut hdr).await?;
    let opcode = hdr[0] & 0x0f;
    let masked = hdr[1] & 0x80 != 0;
    let len = match (hdr[1] & 0x7f) as usize {
        126 => {
            let mut b = [0u8; 2];
            sock.read_exact(&mut b).await?;
            u16::from_be_bytes(b) as usize
        }
        127 => {
            let mut b = [0u8; 8];
            sock.read_exact(&mut b).await?;
            u64::from_be_bytes(b) as usize
        }
        n => n,
    };
    let mask = if masked {
        let mut m = [0u8; 4];
        sock.read_exact(&mut m).await?;
        Some(m)
    } else {
        None
    };
    let mut payload = vec![0u8; len];
    sock.read_exact(&mut payload).await?;
    if let Some(m) = mask {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }
    Ok((opcode, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_matches_rfc3174_vector() {
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn ws_accept_matches_rfc6455_example() {
        // RFC 6455 §1.3 示例：key -> accept 标准答案。
        assert_eq!(
            ws_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
