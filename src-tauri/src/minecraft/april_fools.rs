//! 愚人节版本识别（参考 PCL：内置名单优先，规则只用来兜底未来的新版本）。
//!
//! 前端 `src/aprilFools.ts` 有一份相同的名单与规则，两边需要保持一致。

/// 官方清单（version_manifest_v2.json）中实际存在的愚人节版本 ID（2026-11 核实）。
/// 15w14a 与 26w14a 的 ID 符合常规快照格式，只能靠名单识别。
pub const APRIL_FOOLS_IDS: &[&str] = &[
    "15w14a",               // 2015：The Love and Hugs Update
    "1.RV-Pre1",            // 2016：Trendy Update（发布时间是 3 月 31 日 UTC）
    "3D Shareware v1.34",   // 2019
    "20w14infinite",        // 2020：20w14∞
    "22w13oneblockatatime", // 2022：One Block at a Time
    "23w13a_or_b",          // 2023：The Vote Update
    "24w14potato",          // 2024：Poisonous Potato Update
    "25w14craftmine",       // 2025：Craftmine Update
    "26w14a",               // 2026：Herdcraft Update
];

/// 其他启动器可能使用的别名（文件夹名 / 版本 JSON 的 id）
const ALIASES: &[&str] = &["20w14∞", "20w14infinity", "22w13oneBlockAtATime"];

/// ID 是否在内置名单中（不区分 ASCII 大小写）
pub fn in_list(id: &str) -> bool {
    let id = id.trim();
    APRIL_FOOLS_IDS
        .iter()
        .chain(ALIASES.iter())
        .any(|x| x.eq_ignore_ascii_case(id))
}

/// releaseTime（ISO 8601，例如 2024-04-01T11:07:19+00:00）是否在 3 月 31 日 ~ 4 月 2 日（UTC）之间
fn near_april_first(release_time: &str) -> bool {
    match release_time.get(5..10) {
        Some(md) => md == "03-31" || md == "04-01" || md == "04-02",
        None => false,
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// 常规快照格式 `\d\dw\d\d[a-z]`，例如 24w14a
fn is_weekly_snapshot(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 6
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b'w'
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5].is_ascii_lowercase()
}

/// 版本号式的快照 / 预览版：1.20.5-pre1、1.21-rc1、26.1-snapshot-3、26.4-pre-1、1.14 Pre-Release 2、1.7 …
fn is_numbered_version(id: &str) -> bool {
    let split = id
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map(|(i, _)| i)
        .unwrap_or(id.len());
    let (base, rest) = id.split_at(split);
    if !base.contains('.') || base.starts_with('.') || base.ends_with('.') || base.contains("..") {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    const SUFFIXES: &[&str] = &["-pre-", "-rc-", "-snapshot-", "-pre", "-rc", " Pre-Release "];
    SUFFIXES.iter().any(|p| match rest.strip_prefix(*p) {
        Some(n) => all_digits(n),
        None => false,
    })
}

/// 是否符合常规快照的命名（不符合的才可能被规则判为愚人节版本）
pub fn is_regular_snapshot_id(id: &str) -> bool {
    is_weekly_snapshot(id) || is_numbered_version(id)
}

/// 名单优先；规则兜底：类型是 snapshot、发布于 4 月 1 日前后，并且 ID 不符合任何常规快照格式。
/// 已用 2026-11 的官方清单验证：规则只命中名单中的版本，不会误判正常快照。
pub fn is_april_fools(id: &str, version_type: &str, release_time: &str) -> bool {
    if in_list(id) {
        return true;
    }
    version_type == "snapshot" && near_april_first(release_time) && !is_regular_snapshot_id(id.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_and_rule() {
        assert!(is_april_fools("24w14potato", "snapshot", "2024-04-01T11:07:19+00:00"));
        assert!(is_april_fools("26w14a", "snapshot", "2026-04-01T12:13:30+00:00"));
        assert!(is_april_fools("1.RV-Pre1", "snapshot", "2016-03-31T16:18:53+00:00"));
        // 规则兜底：假想的新愚人节版本
        assert!(is_april_fools("27w14banana", "snapshot", "2027-04-01T12:00:00+00:00"));
        // 4 月 1 日前后的正常快照不能误判
        assert!(!is_april_fools("22w13a", "snapshot", "2022-03-31T14:53:25+00:00"));
        assert!(!is_april_fools("20w14a", "snapshot", "2020-04-02T14:28:06+00:00"));
        assert!(!is_april_fools("26.1.1-rc-1", "snapshot", "2026-03-31T10:07:21+00:00"));
        assert!(!is_april_fools("1.14 Pre-Release 1", "snapshot", "2019-04-01T00:00:00+00:00"));
        assert!(!is_april_fools("1.9.2", "release", "2016-03-30T15:23:55+00:00"));
    }
}
