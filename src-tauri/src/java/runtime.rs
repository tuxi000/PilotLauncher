//! Mojang 官方 Java 运行时清单下载（windows-x64）。
//!
//! 清单：https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json
//! BMCLAPI 镜像使用相同路径（host 替换为 bmclapi2.bangbang93.com），由 download::source 自动处理。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};

use super::detect::JavaInfo;
use super::parse_major;
use crate::download::{download_all, fetch_with, total_size, DownloadItem, DownloadOptions, Verify};
use crate::minecraft::version::DownloadInfo;
use crate::paths::Paths;
use crate::state::Core;
use crate::tasks::TaskHandle;
use crate::util::{read_json, sha1_hex, write_json_atomic};

pub const OFFICIAL_ALL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const PLATFORM: &str = "windows-x64";
const MARKER: &str = ".glass-runtime.json";

#[derive(Deserialize, Clone, Debug)]
struct RuntimeEntry {
    manifest: ManifestRef,
    version: RuntimeVersion,
}

#[derive(Deserialize, Clone, Debug)]
struct ManifestRef {
    #[serde(default)]
    sha1: Option<String>,
    url: String,
}

#[derive(Deserialize, Clone, Debug)]
struct RuntimeVersion {
    name: String,
}

#[derive(Deserialize, Debug)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize, Debug)]
struct RuntimeFile {
    /// file / directory / link
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RuntimeDownloads {
    #[serde(default)]
    raw: Option<DownloadInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Marker {
    component: String,
    version: String,
    major: u32,
}

pub fn runtime_java(dir: &Path) -> PathBuf {
    dir.join("bin").join("javaw.exe")
}

fn check_marker(dir: &Path) -> Option<JavaInfo> {
    let marker: Marker = read_json(&dir.join(MARKER)).ok()?;
    let java = runtime_java(dir);
    if !java.is_file() {
        return None;
    }
    Some(JavaInfo {
        path: java.display().to_string(),
        version: marker.version,
        major: marker.major,
        source: format!("官方运行时 {}", marker.component),
    })
}

/// 查找已安装的运行时：先按组件名，组件名缺失时按主版本。
pub fn find_installed(paths: &Paths, component: Option<&str>, major: u32) -> Option<JavaInfo> {
    let root = paths.runtime_dir();
    if let Some(c) = component {
        return check_marker(&root.join(c));
    }
    std::fs::read_dir(&root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| check_marker(&e.path()))
        .find(|info| info.major == major)
}

/// 选择组件：优先使用版本 JSON 指定的组件，找不到时按 majorVersion 兜底。
fn pick(
    all: &HashMap<String, Vec<RuntimeEntry>>,
    component: Option<&str>,
    major: u32,
) -> anyhow::Result<(String, RuntimeEntry)> {
    if let Some(c) = component {
        if let Some(e) = all.get(c).and_then(|v| v.first()) {
            return Ok((c.to_string(), e.clone()));
        }
    }
    let mut names: Vec<&String> = all.keys().collect();
    // 稳定排序，且非快照组件优先
    names.sort_by_key(|n| (n.contains("snapshot"), n.to_string()));
    let mut best: Option<(u32, String, RuntimeEntry)> = None;
    for name in names {
        let Some(entry) = all.get(name.as_str()).and_then(|v| v.first()) else {
            continue;
        };
        let Some(m) = parse_major(&entry.version.name) else {
            continue;
        };
        if m == major {
            return Ok((name.clone(), entry.clone()));
        }
        if m > major && best.as_ref().map(|b| m < b.0).unwrap_or(true) {
            best = Some((m, name.clone(), entry.clone()));
        }
    }
    best.map(|(_, n, e)| (n, e))
        .ok_or_else(|| anyhow!("找不到适用于 Java {major} 的官方运行时"))
}

/// 确保运行时已安装，必要时下载。`stage` 为 (当前阶段序号, 总阶段数)。
pub async fn ensure_runtime(
    core: &Core,
    task: &Arc<TaskHandle>,
    component: Option<&str>,
    major: u32,
    stage: (u32, u32),
) -> anyhow::Result<JavaInfo> {
    if let Some(info) = find_installed(&core.paths, component, major) {
        return Ok(info);
    }
    let settings = core.settings();
    task.set_stage(stage.0, stage.1, "获取 Java 运行时清单", 0, 0);
    let all: HashMap<String, Vec<RuntimeEntry>> =
        fetch_with(&core.http, &core.sources(OFFICIAL_ALL), settings.retries, |b| {
            let v: serde_json::Value = serde_json::from_slice(b)?;
            let plat = v
                .get(PLATFORM)
                .cloned()
                .ok_or_else(|| anyhow!("运行时清单中没有 {PLATFORM} 平台"))?;
            Ok(serde_json::from_value(plat)?)
        })
        .await
        .context("获取 Java 运行时清单失败")?;

    let (comp, entry) = pick(&all, component, major)?;
    let dir = core.paths.runtime_dir().join(&comp);
    if let Some(info) = check_marker(&dir) {
        return Ok(info);
    }

    let expected = entry.manifest.sha1.clone();
    let manifest: RuntimeManifest =
        fetch_with(&core.http, &core.sources(&entry.manifest.url), settings.retries, |b| {
            if let Some(e) = &expected {
                if !sha1_hex(b).eq_ignore_ascii_case(e) {
                    bail!("运行时文件清单 SHA-1 校验失败");
                }
            }
            Ok(serde_json::from_slice(b)?)
        })
        .await
        .with_context(|| format!("获取 {comp} 文件清单失败"))?;

    let mut items = Vec::new();
    let mut links = Vec::new();
    for (rel, file) in &manifest.files {
        if rel.split('/').any(|c| c == "..") {
            continue;
        }
        let path = dir.join(rel);
        match file.kind.as_str() {
            "directory" => {
                tokio::fs::create_dir_all(&path).await?;
            }
            "file" => {
                if let Some(raw) = file.downloads.as_ref().and_then(|d| d.raw.as_ref()) {
                    items.push(DownloadItem {
                        urls: core.sources(&raw.url),
                        path,
                        sha1: raw.sha1.clone(),
                        size: raw.size,
                    });
                }
            }
            "link" => {
                if let Some(t) = &file.target {
                    links.push((path, t.clone()));
                }
            }
            _ => {}
        }
    }
    if items.is_empty() {
        bail!("运行时 {comp} 的文件清单为空");
    }

    let total = total_size(&items);
    task.set_stage(
        stage.0,
        stage.1,
        format!("下载 Java 运行时 {comp}（{}）", entry.version.name),
        items.len() as u64,
        total,
    );
    download_all(
        &core.http,
        items,
        DownloadOptions {
            concurrency: settings.concurrency,
            retries: settings.retries,
            verify: Verify::Full,
        },
        task,
    )
    .await?;

    // Windows 上几乎没有 link，出现时复制目标文件代替
    for (link, target) in links {
        if let Some(parent) = link.parent() {
            let src = parent.join(&target);
            if src.is_file() && !link.exists() {
                let _ = tokio::fs::copy(&src, &link).await;
            }
        }
    }

    let java = runtime_java(&dir);
    if !java.is_file() {
        bail!("运行时下载完成，但未找到 {}", java.display());
    }
    let marker = Marker {
        component: comp.clone(),
        version: entry.version.name.clone(),
        major: parse_major(&entry.version.name).unwrap_or(major),
    };
    write_json_atomic(&dir.join(MARKER), &marker)?;
    check_marker(&dir).ok_or_else(|| anyhow!("运行时安装校验失败"))
}
