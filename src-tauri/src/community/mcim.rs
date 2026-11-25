//! MCIM 镜像：Modrinth / CurseForge API 与文件的国内镜像。
//!
//! 规则已于 2026-11 对照 https://www.mcimirror.top 与 HMCL（PR #6810）核对：
//!   api.modrinth.com     → mod.mcimirror.top/modrinth
//!   cdn.modrinth.com     → mod.mcimirror.top
//!   api.curseforge.com   → mod.mcimirror.top/curseforge
//!   edge.forgecdn.net    → mod.mcimirror.top
//!   mediafilez.forgecdn.net → mod.mcimirror.top（路径同为 /files/...，官方文档未列出，作为候选之一，失败自动回退）
//! MCIM 的 CurseForge API 不需要 x-api-key，因此启动器**不会**把用户的 Key 发给镜像。

use crate::settings::CommunitySource;

pub const MCIM: &str = "https://mod.mcimirror.top";

const RULES: &[(&str, &str)] = &[
    ("https://api.modrinth.com", "https://mod.mcimirror.top/modrinth"),
    ("https://staging-api.modrinth.com", "https://mod.mcimirror.top/modrinth"),
    ("https://cdn.modrinth.com", "https://mod.mcimirror.top"),
    ("https://api.curseforge.com", "https://mod.mcimirror.top/curseforge"),
    ("https://edge.forgecdn.net", "https://mod.mcimirror.top"),
    ("https://mediafilez.forgecdn.net", "https://mod.mcimirror.top"),
];

pub fn to_mcim(url: &str) -> Option<String> {
    let normalized = match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    };
    RULES.iter().find_map(|(from, to)| {
        normalized
            .strip_prefix(from)
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .map(|rest| format!("{to}{rest}"))
    })
}

/// 按社区资源源策略生成候选地址
pub fn candidates(source: CommunitySource, url: &str) -> Vec<String> {
    let mirror = to_mcim(url);
    let mut out = Vec::with_capacity(2);
    match source {
        CommunitySource::Official => out.push(url.to_string()),
        CommunitySource::MirrorFirst => {
            if let Some(m) = mirror {
                out.push(m);
            }
            out.push(url.to_string());
        }
        CommunitySource::OfficialFirst => {
            out.push(url.to_string());
            if let Some(m) = mirror {
                out.push(m);
            }
        }
    }
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(
            to_mcim("https://api.modrinth.com/v2/search").unwrap(),
            "https://mod.mcimirror.top/modrinth/v2/search"
        );
        assert_eq!(
            to_mcim("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar").unwrap(),
            "https://mod.mcimirror.top/data/AANobbMI/versions/x/sodium.jar"
        );
        assert_eq!(
            to_mcim("https://edge.forgecdn.net/files/1234/567/a.jar").unwrap(),
            "https://mod.mcimirror.top/files/1234/567/a.jar"
        );
        assert!(to_mcim("https://example.com").is_none());
        let c = candidates(CommunitySource::MirrorFirst, "https://api.curseforge.com/v1/mods/1");
        assert!(c[0].starts_with(MCIM));
        assert_eq!(candidates(CommunitySource::Official, "https://api.curseforge.com/v1/mods/1").len(), 1);
    }
}
