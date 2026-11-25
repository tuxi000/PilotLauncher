//! 下载源与 URL 替换规则。

use crate::settings::DownloadSource;

pub const BMCLAPI: &str = "https://bmclapi2.bangbang93.com";

/// 官方地址 → BMCLAPI 镜像地址的前缀替换表
const MIRROR_RULES: &[(&str, &str)] = &[
    ("https://piston-meta.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://launchermeta.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://piston-data.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://launcher.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://resources.download.minecraft.net", "https://bmclapi2.bangbang93.com/assets"),
    ("https://libraries.minecraft.net", "https://bmclapi2.bangbang93.com/maven"),
    // 以下为模组加载器（Forge / NeoForge / Fabric / Quilt）与 authlib-injector 所需
    ("https://maven.minecraftforge.net", "https://bmclapi2.bangbang93.com/maven"),
    ("https://files.minecraftforge.net/maven", "https://bmclapi2.bangbang93.com/maven"),
    ("https://maven.neoforged.net/releases", "https://bmclapi2.bangbang93.com/maven"),
    ("https://maven.fabricmc.net", "https://bmclapi2.bangbang93.com/maven"),
    ("https://meta.fabricmc.net", "https://bmclapi2.bangbang93.com/fabric-meta"),
    ("https://maven.quiltmc.org/repository/release", "https://bmclapi2.bangbang93.com/maven"),
    ("https://authlib-injector.yushi.moe", "https://bmclapi2.bangbang93.com/mirrors/authlib-injector"),
];

/// 把官方地址转换为 BMCLAPI 镜像地址；无法转换时返回 None。
pub fn to_mirror(url: &str) -> Option<String> {
    let normalized = if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    };
    for (from, to) in MIRROR_RULES {
        if let Some(rest) = normalized.strip_prefix(from) {
            if rest.is_empty() || rest.starts_with('/') {
                return Some(format!("{to}{rest}"));
            }
        }
    }
    None
}

/// 根据下载源设置生成按优先级排列的候选地址列表。
pub fn candidates(source: DownloadSource, url: &str) -> Vec<String> {
    let mirror = to_mirror(url);
    let mut out = Vec::with_capacity(2);
    match source {
        DownloadSource::Official => out.push(url.to_string()),
        DownloadSource::Bmclapi => {
            if let Some(m) = mirror {
                out.push(m);
            }
            out.push(url.to_string());
        }
        DownloadSource::OfficialFirst => {
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
    fn mirror_rules() {
        assert_eq!(
            to_mirror("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json").unwrap(),
            "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
        );
        assert_eq!(
            to_mirror("https://resources.download.minecraft.net/ab/abcdef").unwrap(),
            "https://bmclapi2.bangbang93.com/assets/ab/abcdef"
        );
        assert_eq!(
            to_mirror("https://libraries.minecraft.net/com/mojang/x.jar").unwrap(),
            "https://bmclapi2.bangbang93.com/maven/com/mojang/x.jar"
        );
        assert!(to_mirror("https://example.com/a").is_none());
    }

    #[test]
    fn candidate_order() {
        let u = "https://launchermeta.mojang.com/v1/x.json";
        let c = candidates(DownloadSource::OfficialFirst, u);
        assert_eq!(c[0], u);
        assert!(c[1].starts_with(BMCLAPI));
        let c = candidates(DownloadSource::Bmclapi, u);
        assert!(c[0].starts_with(BMCLAPI));
        assert_eq!(candidates(DownloadSource::Official, u).len(), 1);
    }
}
