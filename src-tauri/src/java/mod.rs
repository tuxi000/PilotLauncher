//! Java 运行时：官方运行时自动下载 + 本机 Java 检测。

pub mod detect;
pub mod runtime;

pub use detect::JavaInfo;

/// 从版本号字符串中解析 Java 主版本（"1.8.0_51" → 8，"17.0.8" → 17，"8u51" → 8）
pub fn parse_major(name: &str) -> Option<u32> {
    let s = name.trim().trim_matches('"');
    let s = s.strip_prefix("1.").unwrap_or(s);
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_major;

    #[test]
    fn majors() {
        assert_eq!(parse_major("1.8.0_51"), Some(8));
        assert_eq!(parse_major("17.0.8"), Some(17));
        assert_eq!(parse_major("8u51"), Some(8));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(parse_major("abc"), None);
    }
}
