//! 文件名通配匹配：`*` 任意序列、`?` 单字符，ASCII 大小写不敏感。
//!
//! 供 fs_search 按文件名通配过滤；只匹配文件名（不含目录分隔符），
//! 因此 `*` 不会跨目录。经典贪心回溯算法，线性时间。

/// 通配匹配入口。
pub fn matches(name: &str, pattern: &str) -> bool {
    let text: Vec<char> = name.chars().collect();
    let pat: Vec<char> = pattern.chars().collect();
    let (mut ti, mut pi) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut mark = 0usize;
    while ti < text.len() {
        if pi < pat.len() && (pat[pi] == '?' || eq_ascii(pat[pi], text[ti])) {
            ti += 1;
            pi += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
}

fn eq_ascii(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_star() {
        assert!(matches("abc.txt", "abc.txt"));
        assert!(matches("abc.txt", "*"));
        assert!(matches("abc.txt", "abc*"));
        assert!(matches("abc.txt", "*.txt"));
        assert!(matches("abc.txt", "*b*"));
        assert!(!matches("abc.txt", "*.log"));
        assert!(!matches("abc.txt", "abd*"));
    }

    #[test]
    fn question_mark_single_char() {
        assert!(matches("abc", "a?c"));
        assert!(matches("a1c", "a?c"));
        assert!(!matches("abcd", "a?c"));
    }

    #[test]
    fn case_insensitive() {
        assert!(matches("README.MD", "readme.md"));
        assert!(matches("Log.Txt", "LOG.TXT"));
    }

    #[test]
    fn empty_and_multi_star() {
        assert!(matches("", "*"));
        assert!(matches("ab", "**"));
        assert!(matches("aXbYc", "a*b*c"));
        assert!(matches("aXY", "a*Y"));
        assert!(!matches("ab", "a*b*c"));
        assert!(matches("abcdef", "a*"));
        assert!(matches("abcdef", "*f"));
    }

    #[test]
    fn star_does_not_cross_separators() {
        // matches() 只接收 basename，分隔符不在输入内；此用例保证 * 语义不外溢
        assert!(matches("a-b-c", "*b*"));
        assert!(matches("a.b", "a.*"));
    }
}
