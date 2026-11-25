//! 版本清单（version_manifest_v2.json）。

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::state::Core;
use crate::download::fetch_with;
use crate::util::{read_json, write_json_atomic};

use super::april_fools;

pub const OFFICIAL_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Latest {
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub snapshot: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    /// release / snapshot / old_beta / old_alpha（保持官方原值，愚人节版本仍是 snapshot）
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
    /// 是否为愚人节版本（启动器计算，官方清单没有这个字段；见 april_fools.rs）
    #[serde(default)]
    pub april_fools: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct VersionManifest {
    #[serde(default)]
    pub latest: Latest,
    #[serde(default)]
    pub versions: Vec<ManifestVersion>,
}

/// 标记愚人节版本（网络获取与磁盘缓存两条路径都要调用）
fn mark_april_fools(m: &mut VersionManifest) {
    for v in m.versions.iter_mut() {
        v.april_fools = april_fools::is_april_fools(&v.id, &v.version_type, &v.release_time);
    }
}

const CACHE_TTL: Duration = Duration::from_secs(600);

/// 获取版本清单：内存缓存 10 分钟；网络失败时回退到磁盘缓存。
pub async fn get_manifest(core: &Core, force: bool) -> anyhow::Result<VersionManifest> {
    if !force {
        let cache = core.manifest_cache.lock().await;
        if let Some((t, m)) = cache.as_ref() {
            if t.elapsed() < CACHE_TTL {
                return Ok(m.clone());
            }
        }
    }
    let cache_file = core.paths.cache_dir().join("version_manifest_v2.json");
    let urls = core.sources(OFFICIAL_MANIFEST);
    let fetched = fetch_with(&core.http, &urls, core.settings().retries, |b| {
        Ok(serde_json::from_slice::<VersionManifest>(b)?)
    })
    .await;
    match fetched {
        Ok(mut m) => {
            mark_april_fools(&mut m);
            let _ = write_json_atomic(&cache_file, &m);
            *core.manifest_cache.lock().await = Some((Instant::now(), m.clone()));
            Ok(m)
        }
        Err(e) => match read_json::<VersionManifest>(&cache_file) {
            Ok(mut m) => {
                mark_april_fools(&mut m);
                Ok(m)
            }
            Err(_) => Err(e.context("获取版本列表失败，请检查网络或切换下载源")),
        },
    }
}
