//! 共享匹配原语：词边界匹配与路径归一化（纯字符串逻辑，无 IO）。
//!
//! 全部匹配大小写不敏感、以字母数字为词边界分词——执法判定的统一底座，
//! 红线检测与风险分级共用，避免各模块各自实现导致口径漂移。

/// hay 中是否存在 needle 的全部词（连续出现即命中），大小写不敏感。
///
/// hay 与 needle 都先做 camelCase 拆词再匹配；needle 的每个词在 hay 中
/// 命中一片连续的词段（段拼接 == 该词，含独立词与拆词后的拼接两种形态），
/// 词段按顺序紧邻。覆盖 "MpPreference" 对 "mppreference"、"OpenSSL" 对
/// "openssl" 这类拼写差异，同时保持 "transformer" 不含 "format" 的词边界。
pub(crate) fn contains_words(hay: &str, needle: &str) -> bool {
    let hwords = words(hay);
    let nwords = words(needle);
    if nwords.is_empty() || hwords.len() < nwords.len() {
        return false;
    }
    (0..hwords.len()).any(|start| match_from(&hwords, start, &nwords))
}

/// 从 hwords[start..] 依次消化 nwords 中每个词（各占一片连续词段）。
fn match_from(hwords: &[String], start: usize, nwords: &[String]) -> bool {
    if nwords.is_empty() {
        return true;
    }
    for end in start + 1..=hwords.len() {
        if hwords[start..end].concat() != nwords[0] {
            continue;
        }
        if match_from(hwords, end, &nwords[1..]) {
            return true;
        }
    }
    false
}

/// hay 是否命中 needles 中任一模式（[contains_words] 语义）。
pub(crate) fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| contains_words(hay, n))
}

/// 按空格拆分命令行为 argv 数组（空 token 丢弃）。
pub fn split_words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

/// 路径轻量归一化：小写、统一分隔符为 '/'、折叠 "." 与 ".."、
/// 剥离 Windows 设备前缀（\\?\ 与 \\.\）、去掉末尾斜杠。
/// 纯词法处理，不做真实文件系统 canonicalize（属 T23 路径监狱职责）。
pub(crate) fn norm_path(input: &str) -> String {
    let mut s = input.trim().to_ascii_lowercase().replace('\\', "/");
    for pre in ["//?/", "//./", "//?/"] {
        while s.starts_with(pre) {
            s = s[pre.len()..].to_string();
        }
    }
    let mut segs: Vec<&str> = Vec::new();
    for seg in s.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            other => segs.push(other),
        }
    }
    if segs.is_empty() {
        return "/".to_string();
    }
    let mut out = segs.join("/");
    while out.ends_with('/') {
        out.pop();
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

fn words(s: &str) -> Vec<String> {
    let camel = camel_split(s);
    camel
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

/// camelCase / PascalCase 拆词：小写或数字后紧跟大写处插入空格。
/// "DisableRealtimeMonitoring" -> "Disable Realtime Monitoring"。
fn camel_split(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower_digit = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() && prev_lower_digit {
            out.push(' ');
        }
        out.push(ch);
        prev_lower_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(hay: &str, needle: &str) -> bool {
        contains_words(hay, needle)
    }

    #[test]
    fn single_word_boundary() {
        assert!(w("run format c:", "format"));
        assert!(!w("run transformer", "format"));
        assert!(!w("model check", "del"));
        assert!(w("DEL /q a.txt", "del"));
    }

    #[test]
    fn multi_word_consecutive() {
        assert!(w("openssl enc -aes256 in.txt", "openssl enc"));
        assert!(!w("openssl dgst sha256", "openssl enc"));
        assert!(w(
            "Set-MpPreference -DisableRealtimeMonitoring",
            "set-mppreference"
        ));
    }

    #[test]
    fn path_norm_forms() {
        assert_eq!(norm_path("C:\\Users\\Jane\\x.txt"), "c:/users/jane/x.txt");
        assert_eq!(
            norm_path("C:/Users/Jane/../Jane/.ssh/id_rsa"),
            "c:/users/jane/.ssh/id_rsa"
        );
        assert_eq!(norm_path("\\\\?\\C:\\a"), "c:/a");
        assert_eq!(norm_path("/"), "/");
        assert_eq!(norm_path("c:\\"), "c:");
    }
}
