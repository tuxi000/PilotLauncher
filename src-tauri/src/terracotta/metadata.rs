//! 固定的 Terracotta | 陶瓦联机 版本信息。
//!
//! 版本与哈希和 HMCL 当前 main 分支内置的 `assets/terracotta.json` 一致（version_latest = 0.4.2），
//! 包的 SHA-256 来自 GitHub Releases API 的 `digest` 字段，SHA-512 与 HMCL 元数据一致，并已人工下载核对：
//! GitHub / Gitee / CNB 三处下载的文件逐字节相同。
//!
//! Windows x86_64 的发行包是 `terracotta-<版本>-windows-x86_64-pkg.tar.gz`（GNU tar + gzip），
//! 里面只有两个文件：`terracotta-<版本>-windows-x86_64.exe` 与 `VCRUNTIME140.DLL`。
//! 两个文件原样解压使用，**绝不修改**（AGPL 例外条款要求与“未修改的程序”交互）。

use crate::settings::DownloadSource;

pub const VERSION: &str = "0.4.2";
pub const CLASSIFIER: &str = "windows-x86_64";
pub const PACKAGE_NAME: &str = "terracotta-0.4.2-windows-x86_64-pkg.tar.gz";
/// 安装包大小（字节）
pub const PACKAGE_SIZE: u64 = 8_135_914;
pub const PACKAGE_SHA256: &str = "07ebe139e3ca5f74576e58b1a96efe59abdfbe148d3f1a49bfdca8b6f70745f0";
pub const PACKAGE_SHA512: &str = "6a98f524d4f00373696517306af8aa50d01d55ce4eadb27e9e4bc2f882707a0b5f20d5d4c33371d1459dcf5bf144ffed9beb414202d9ccf32b11dbbfcf19d650";

/// 主程序文件名
pub const EXE_NAME: &str = "terracotta-0.4.2-windows-x86_64.exe";

pub struct BundleFile {
    pub name: &'static str,
    pub size: u64,
    pub sha512: &'static str,
}

/// 安装包内需要的文件（逐个校验 SHA-512）
pub const FILES: &[BundleFile] = &[
    BundleFile {
        name: "terracotta-0.4.2-windows-x86_64.exe",
        size: 9_402_880,
        sha512: "6e98d1f2380ed22fb5a2dd4aafce6c773e9cf69100c8bb8e49e7d6983756bdb9a31f80e06bcfbe5a2742144fe806d3d687dec54d8f09d87c659341f99dd9fd80",
    },
    BundleFile {
        name: "VCRUNTIME140.DLL",
        size: 97_656,
        sha512: "3d4b24061f72c0e957c7b04a0c4098c94c8f1afb4a7e159850b9939c7210d73398be6f27b5ab85073b4e8c999816e7804fef0f6115c39cd061f4aaeb4dcda8cf",
    },
];

/// 官方：GitHub Releases
pub const OFFICIAL_URL: &str =
    "https://github.com/burningtnt/Terracotta/releases/download/v0.4.2/terracotta-0.4.2-windows-x86_64-pkg.tar.gz";

/// 国内镜像：Gitee（README 中的“国内镜像”）与 CNB（HMCL 元数据中的 downloads_CN）
pub const MIRROR_URLS: &[(&str, &str)] = &[
    (
        "Gitee 镜像",
        "https://gitee.com/burningtnt/Terracotta/releases/download/v0.4.2/terracotta-0.4.2-windows-x86_64-pkg.tar.gz",
    ),
    (
        "CNB 镜像",
        "https://cnb.cool/HMCL-Terracotta/Terracotta/-/releases/download/v0.4.2/terracotta-0.4.2-windows-x86_64-pkg.tar.gz",
    ),
];

pub const REPO_URL: &str = "https://github.com/burningtnt/Terracotta";
pub const RELEASE_URL: &str = "https://github.com/burningtnt/Terracotta/releases/tag/v0.4.2";
pub const LICENSE_URL: &str = "https://github.com/burningtnt/Terracotta/blob/master/LICENSE";
pub const DOCS_URL: &str = "https://burningtnt.github.io/Terracotta/";
pub const EASYTIER_URL: &str = "https://github.com/EasyTier/EasyTier";

/// 按下载源设置生成 (名称, 地址) 列表：
/// - 官方：只用 GitHub
/// - BMCLAPI（镜像优先）：陶瓦没有 BMCLAPI 镜像，按“国内镜像优先”处理：Gitee → CNB → GitHub
/// - 官方优先：GitHub → Gitee → CNB
pub fn sources(source: DownloadSource) -> Vec<(String, String)> {
    let official = ("GitHub Releases".to_string(), OFFICIAL_URL.to_string());
    let mirrors: Vec<(String, String)> = MIRROR_URLS
        .iter()
        .map(|(label, url)| (label.to_string(), url.to_string()))
        .collect();
    let mut out: Vec<(String, String)> = Vec::new();
    match source {
        DownloadSource::Official => out.push(official),
        DownloadSource::Bmclapi => {
            out.extend(mirrors);
            out.push(official);
        }
        DownloadSource::OfficialFirst => {
            out.push(official);
            out.extend(mirrors);
        }
    }
    out
}

pub fn download_urls(source: DownloadSource) -> Vec<String> {
    sources(source).into_iter().map(|(_, url)| url).collect()
}

pub fn source_labels(source: DownloadSource) -> Vec<String> {
    sources(source).into_iter().map(|(label, _)| label).collect()
}
