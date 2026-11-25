//! 资源索引（asset index）与资源对象（objects）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::Deserialize;

use super::version::AssetIndexRef;
use crate::state::Core;
use crate::download::{fetch_with, DownloadItem};
use crate::util::sha1_hex;

pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Deserialize, Clone, Debug, Default)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    /// 1.6 之前：资源需要复制到 <gameDir>/resources
    #[serde(default)]
    pub map_to_resources: bool,
    /// 1.6 ~ 1.7.2：资源需要复制到 assets/virtual/legacy
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub fn object_rel(hash: &str) -> String {
    let prefix = hash.get(..2).unwrap_or("00");
    format!("{prefix}/{hash}")
}

pub fn index_path(assets_dir: &Path, id: &str) -> PathBuf {
    assets_dir.join("indexes").join(format!("{id}.json"))
}

/// 读取本地资源索引；不存在或校验失败时重新下载。
pub async fn ensure_index(core: &Core, idx: &AssetIndexRef, assets_dir: &Path) -> anyhow::Result<AssetIndex> {
    let path = index_path(assets_dir, &idx.id);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        let sha_ok = idx
            .sha1
            .as_ref()
            .map(|s| sha1_hex(&bytes).eq_ignore_ascii_case(s))
            .unwrap_or(true);
        if sha_ok {
            if let Ok(index) = serde_json::from_slice::<AssetIndex>(&bytes) {
                return Ok(index);
            }
        }
    }
    let urls = core.sources(&idx.url);
    let expected = idx.sha1.clone();
    let bytes = fetch_with(&core.http, &urls, core.settings().retries, |b| {
        if let Some(e) = &expected {
            if !sha1_hex(b).eq_ignore_ascii_case(e) {
                bail!("资源索引 SHA-1 校验失败");
            }
        }
        serde_json::from_slice::<AssetIndex>(b).context("资源索引格式错误")?;
        Ok(b.to_vec())
    })
    .await
    .with_context(|| format!("下载资源索引 {} 失败", idx.id))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn object_items(
    index: &AssetIndex,
    assets_dir: &Path,
    sources: &dyn Fn(&str) -> Vec<String>,
) -> Vec<DownloadItem> {
    let objects_dir = assets_dir.join("objects");
    let mut seen = std::collections::HashSet::new();
    index
        .objects
        .values()
        .filter(|o| o.hash.len() >= 2 && seen.insert(o.hash.clone()))
        .map(|o| {
            let rel = object_rel(&o.hash);
            DownloadItem {
                urls: sources(format!("{RESOURCES_URL}/{rel}").as_str()),
                path: objects_dir.join(&rel),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
            }
        })
        .collect()
}

/// 为旧版本准备资源目录，返回 ${game_assets} 应指向的目录。
pub async fn prepare_legacy_assets(
    index: AssetIndex,
    assets_dir: PathBuf,
    game_dir: PathBuf,
) -> anyhow::Result<Option<PathBuf>> {
    if !index.is_virtual && !index.map_to_resources {
        return Ok(None);
    }
    tokio::task::spawn_blocking(move || -> anyhow::Result<Option<PathBuf>> {
        let target = if index.map_to_resources {
            game_dir.join("resources")
        } else {
            assets_dir.join("virtual").join("legacy")
        };
        let objects = assets_dir.join("objects");
        for (name, obj) in &index.objects {
            if name.split('/').any(|c| c == "..") {
                continue;
            }
            let dest = target.join(name);
            if dest.is_file() {
                continue;
            }
            let src = objects.join(object_rel(&obj.hash));
            if !src.is_file() {
                continue;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dest)
                .with_context(|| format!("复制旧版资源失败：{}", dest.display()))?;
        }
        Ok(Some(target))
    })
    .await?
}
