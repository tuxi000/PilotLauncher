//! authlib-injector.jar 自动下载。
//!
//! 官方下载 API：https://authlib-injector.yushi.moe/artifact/latest.json
//! BMCLAPI 镜像：https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};

use crate::download::{fetch_with, source};
use crate::settings::DownloadSource;
use crate::state::Core;
use crate::util::sha256_hex;

const OFFICIAL_LATEST: &str = "https://authlib-injector.yushi.moe/artifact/latest.json";
const MIRROR_LATEST: &str = "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 3600);

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Latest {
    #[serde(default)]
    build_number: u32,
    #[serde(default)]
    version: String,
    download_url: String,
    checksums: Checksums,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Checksums {
    sha256: String,
}

fn recently_checked(path: &PathBuf) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|d| d < CHECK_INTERVAL)
        .unwrap_or(false)
}

/// 返回本地 authlib-injector.jar 路径（必要时下载或更新）
pub async fn ensure_authlib(core: &Core) -> anyhow::Result<PathBuf> {
    let dir = core.paths.authlib_dir();
    let jar = dir.join("authlib-injector.jar");
    let info_file = dir.join("latest.json");
    if jar.is_file() && recently_checked(&info_file) {
        return Ok(jar);
    }
    let urls: Vec<String> = match core.settings().download_source {
        DownloadSource::Official => vec![OFFICIAL_LATEST.into(), MIRROR_LATEST.into()],
        _ => vec![MIRROR_LATEST.into(), OFFICIAL_LATEST.into()],
    };
    let latest = fetch_with(&core.http, &urls, 1, |b| Ok(serde_json::from_slice::<Latest>(b)?)).await;
    let latest = match latest {
        Ok(l) => l,
        Err(e) => {
            if jar.is_file() {
                return Ok(jar);
            }
            return Err(e.context("获取 authlib-injector 版本信息失败"));
        }
    };

    tokio::fs::create_dir_all(&dir).await?;
    if let Ok(existing) = tokio::fs::read(&jar).await {
        if sha256_hex(&existing).eq_ignore_ascii_case(&latest.checksums.sha256) {
            tokio::fs::write(&info_file, serde_json::to_vec_pretty(&latest)?).await?;
            return Ok(jar);
        }
    }

    let mut jar_urls = vec![latest.download_url.clone()];
    if let Some(m) = source::to_mirror(&latest.download_url) {
        jar_urls.push(m);
    }
    let expected = latest.checksums.sha256.clone();
    let bytes = fetch_with(&core.http, &jar_urls, 2, |b| {
        if !sha256_hex(b).eq_ignore_ascii_case(&expected) {
            bail!("authlib-injector SHA-256 校验失败");
        }
        Ok(b.to_vec())
    })
    .await
    .with_context(|| format!("下载 authlib-injector {} 失败", latest.version))?;
    tokio::fs::write(&jar, &bytes).await?;
    tokio::fs::write(&info_file, serde_json::to_vec_pretty(&latest)?).await?;
    if !jar.is_file() {
        return Err(anyhow!("authlib-injector 写入失败"));
    }
    Ok(jar)
}
