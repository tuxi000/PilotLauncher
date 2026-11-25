//! 依赖库解析：Maven 坐标、规则过滤、natives 识别、下载项生成。

use std::path::{Path, PathBuf};

use super::rules::{rules_allow, Features};
use super::version::{Library, VersionJson};
use crate::download::DownloadItem;

pub const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

#[derive(Clone, Debug)]
pub struct MavenCoord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub ext: String,
}

impl MavenCoord {
    pub fn parse(name: &str) -> Option<Self> {
        let (main, ext) = match name.split_once('@') {
            Some((m, e)) => (m, e.to_string()),
            None => (name, "jar".to_string()),
        };
        let parts: Vec<&str> = main.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Self {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).map(|s| s.to_string()),
            ext,
        })
    }

    pub fn with_classifier(&self, classifier: &str) -> Self {
        let mut c = self.clone();
        c.classifier = Some(classifier.to_string());
        c
    }

    /// Maven 相对路径，例如 org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar
    pub fn path(&self) -> String {
        let classifier = self
            .classifier
            .as_ref()
            .map(|c| format!("-{c}"))
            .unwrap_or_default();
        format!(
            "{}/{}/{}/{}-{}{}.{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            self.artifact,
            self.version,
            classifier,
            self.ext
        )
    }

    /// 去重用的键：不含版本号，子版本的库会覆盖父版本的同名库
    pub fn key(&self) -> String {
        match &self.classifier {
            Some(c) => format!("{}:{}:{}", self.group, self.artifact, c),
            None => format!("{}:{}", self.group, self.artifact),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ExtractKind {
    /// 普通库
    None,
    /// 旧式 natives（classifiers），保留目录结构并遵守 exclude
    Legacy(Vec<String>),
    /// 1.19+ 新式 natives（natives-windows 坐标），只平铺解压 dll
    FlatDll,
}

#[derive(Clone, Debug)]
pub struct ResolvedLibrary {
    pub key: String,
    pub path: PathBuf,
    pub download: Option<DownloadItem>,
    pub on_classpath: bool,
    pub extract: ExtractKind,
}

/// 是否为本机（Windows x64）可用的新式 natives 分类器
fn native_classifier_ok(c: &str) -> bool {
    matches!(c, "natives-windows" | "natives-windows-x86_64" | "natives-windows-64")
}

fn join_url(base: &str, rel: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{rel}")
    } else {
        format!("{base}/{rel}")
    }
}

fn make_item(
    path: &Path,
    url: Option<String>,
    sha1: Option<String>,
    size: Option<u64>,
    sources: &dyn Fn(&str) -> Vec<String>,
) -> Option<DownloadItem> {
    let url = url.filter(|u| !u.trim().is_empty())?;
    Some(DownloadItem {
        urls: sources(url.as_str()),
        path: path.to_path_buf(),
        sha1,
        size,
    })
}

pub fn resolve_libraries(
    version: &VersionJson,
    lib_dir: &Path,
    features: &Features,
    sources: &dyn Fn(&str) -> Vec<String>,
) -> Vec<ResolvedLibrary> {
    let mut out = Vec::new();
    for lib in &version.libraries {
        resolve_one(lib, lib_dir, features, sources, &mut out);
    }
    out
}

fn resolve_one(
    lib: &Library,
    lib_dir: &Path,
    features: &Features,
    sources: &dyn Fn(&str) -> Vec<String>,
    out: &mut Vec<ResolvedLibrary>,
) {
    if !rules_allow(lib.rules.as_ref(), features) {
        return;
    }
    let Some(coord) = MavenCoord::parse(&lib.name) else {
        return;
    };
    let mut new_style_native = false;
    if let Some(c) = &coord.classifier {
        if c.starts_with("natives-") {
            if !native_classifier_ok(c) {
                // 其他平台 / 架构（linux、macos、x86、arm64）的 natives
                return;
            }
            new_style_native = true;
        }
    }

    let legacy_classifier = lib
        .natives
        .as_ref()
        .and_then(|n| n.get("windows"))
        .map(|c| c.replace("${arch}", "64"));
    let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());
    let base_url = lib.url.clone().unwrap_or_else(|| MOJANG_LIBRARIES.to_string());

    // 主构件：有 artifact，或者没有 downloads 字段的 Maven 风格库（纯 natives 库除外）
    let add_main = artifact.is_some() || (lib.downloads.is_none() && legacy_classifier.is_none());
    if add_main {
        let rel = artifact
            .and_then(|a| a.path.clone())
            .unwrap_or_else(|| coord.path());
        let path = lib_dir.join(&rel);
        let url = match artifact {
            Some(a) => a.url.clone(),
            None => Some(join_url(&base_url, &coord.path())),
        };
        let (sha1, size) = match artifact {
            Some(a) => (a.sha1.clone(), a.size),
            None => (lib.sha1.clone(), lib.size),
        };
        let download = make_item(&path, url, sha1, size, sources);
        out.push(ResolvedLibrary {
            key: coord.key(),
            path,
            download,
            on_classpath: true,
            extract: if new_style_native { ExtractKind::FlatDll } else { ExtractKind::None },
        });
    }

    if let Some(classifier) = legacy_classifier {
        let native_coord = coord.with_classifier(&classifier);
        let classified = lib
            .downloads
            .as_ref()
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier));
        let rel = classified
            .and_then(|a| a.path.clone())
            .unwrap_or_else(|| native_coord.path());
        let path = lib_dir.join(&rel);
        let url = match classified {
            Some(a) => a.url.clone(),
            None => Some(join_url(&base_url, &native_coord.path())),
        };
        let download = make_item(
            &path,
            url,
            classified.and_then(|a| a.sha1.clone()),
            classified.and_then(|a| a.size),
            sources,
        );
        let exclude = lib
            .extract
            .as_ref()
            .map(|e| e.exclude.clone())
            .unwrap_or_default();
        out.push(ResolvedLibrary {
            key: format!("{}#natives", native_coord.key()),
            path,
            download,
            on_classpath: false,
            extract: ExtractKind::Legacy(exclude),
        });
    }
}

/// 生成 classpath 条目（按 key 去重，先出现者优先），客户端 jar 放在最后。
pub fn classpath(libs: &[ResolvedLibrary], client_jar: &Path) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for lib in libs.iter().filter(|l| l.on_classpath) {
        if seen.insert(lib.key.clone()) {
            out.push(lib.path.clone());
        }
    }
    out.push(client_jar.to_path_buf());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_path() {
        let c = MavenCoord::parse("org.lwjgl:lwjgl:3.3.1:natives-windows").unwrap();
        assert_eq!(c.path(), "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar");
        let c = MavenCoord::parse("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap();
        assert_eq!(c.path(), "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
    }

    #[test]
    fn legacy_natives() {
        let json = r#"{
            "id": "1.8.9",
            "libraries": [{
                "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"},
                "extract": {"exclude": ["META-INF/"]},
                "downloads": {"classifiers": {"natives-windows": {
                    "path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209-natives-windows.jar",
                    "url": "https://libraries.minecraft.net/x.jar", "sha1": "abc", "size": 10}}}
            }, {
                "name": "org.lwjgl:lwjgl:3.3.1:natives-windows-x86",
                "downloads": {"artifact": {"path": "a.jar", "url": "https://libraries.minecraft.net/a.jar"}}
            }]
        }"#;
        let v: VersionJson = serde_json::from_str(json).unwrap();
        let libs = resolve_libraries(&v, Path::new("libs"), &Features::default(), &|u| vec![u.to_string()]);
        assert_eq!(libs.len(), 1);
        assert!(!libs[0].on_classpath);
        assert!(matches!(libs[0].extract, ExtractKind::Legacy(_)));
    }
}
