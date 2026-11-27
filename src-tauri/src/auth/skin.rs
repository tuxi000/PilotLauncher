//! 账户皮肤。
//!
//! - 外置登录账户：请求认证服务器的会话接口
//!   `{API 根地址}/sessionserver/session/minecraft/profile/{无连字符 UUID}`，
//!   取 `properties` 里 `name = textures` 的值，base64 解码后得到
//!   `textures.SKIN.url`（`metadata.model = "slim"` 表示纤细手臂）和 `textures.CAPE.url`。
//!   贴图由后端下载并缓存到 `<数据目录>/cache/skins/<URL 的 SHA-1>.png`，以 data URL 交给前端，
//!   这样前端的 canvas / WebGL 不会遇到跨域问题，没有网络时也能显示上次的皮肤。
//! - 离线账户没有皮肤，前端按 UUID 显示默认的 Steve / Alex（规则见 src/skin.ts）。

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::store::{Account, AccountKind};
use crate::state::Core;
use crate::util::{now_ms, read_json, sha1_hex, write_json_atomic};

/// 缓存目录名（`cache/` 下）。清理“启动器缓存”时保留这个目录
pub const CACHE_DIR: &str = "skins";
/// 皮肤 / 披风贴图的大小上限
const MAX_TEXTURE: usize = 2 * 1024 * 1024;
/// 角色信息（皮肤地址）的缓存时间
const PROFILE_TTL_MS: u64 = 10 * 60 * 1000;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkinView {
    /// 皮肤贴图的 data URL；为空时前端显示默认皮肤
    pub skin: Option<String>,
    /// 披风贴图的 data URL
    pub cape: Option<String>,
    /// 纤细手臂（Alex 模型）
    pub slim: bool,
    /// yggdrasil：来自认证服务器；default：没有自定义皮肤
    pub source: String,
    /// 获取失败的原因（只用于提示，失败时仍然显示默认皮肤）
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct ProfileCache {
    skin_url: Option<String>,
    cape_url: Option<String>,
    slim: bool,
    fetched_at: u64,
}

fn default_view(error: Option<String>) -> SkinView {
    SkinView {
        source: "default".into(),
        error,
        ..Default::default()
    }
}

/// 读取账户皮肤。任何失败都不报错，只是退回默认皮肤（error 里写明原因）
pub async fn account_skin(core: &Core, account: &Account, force: bool) -> SkinView {
    if account.kind != AccountKind::Yggdrasil {
        return default_view(None);
    }
    match yggdrasil_skin(core, account, force).await {
        Ok(v) => v,
        Err(e) => default_view(Some(format!("{e:#}"))),
    }
}

async fn yggdrasil_skin(core: &Core, account: &Account, force: bool) -> anyhow::Result<SkinView> {
    let root = account
        .server_url
        .clone()
        .ok_or_else(|| anyhow!("外置账户缺少认证服务器地址"))?;
    let dir = core.paths.skins_dir();
    let uuid = account.uuid.replace('-', "");
    let key = sha1_hex(format!("{root}|{uuid}").as_bytes());
    let index = dir.join(format!("profile-{}.json", &key[..16]));

    let cached: Option<ProfileCache> = read_json(&index).ok();
    let fresh = cached
        .as_ref()
        .map(|c| now_ms().saturating_sub(c.fetched_at) < PROFILE_TTL_MS)
        .unwrap_or(false);
    let profile = if fresh && !force {
        cached.unwrap_or_default()
    } else {
        match fetch_profile(&core.http, &root, &uuid).await {
            Ok(p) => {
                let _ = write_json_atomic(&index, &p);
                p
            }
            // 网络不通时用上次的结果
            Err(e) => match cached {
                Some(c) => c,
                None => return Err(e),
            },
        }
    };

    let mut error: Option<String> = None;
    let skin = match profile.skin_url.as_deref() {
        Some(url) => match texture_data_url(&core.http, &dir, url).await {
            Ok(d) => Some(d),
            Err(e) => {
                error = Some(format!("{e:#}"));
                None
            }
        },
        None => None,
    };
    let cape = match profile.cape_url.as_deref() {
        Some(url) => texture_data_url(&core.http, &dir, url).await.ok(),
        None => None,
    };
    let source = if skin.is_some() { "yggdrasil" } else { "default" };
    Ok(SkinView {
        skin,
        cape,
        slim: profile.slim,
        source: source.to_string(),
        error,
    })
}

async fn fetch_profile(http: &reqwest::Client, root: &str, uuid: &str) -> anyhow::Result<ProfileCache> {
    let url = format!(
        "{}/sessionserver/session/minecraft/profile/{uuid}",
        root.trim_end_matches('/')
    );
    let resp = http
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .with_context(|| format!("无法连接认证服务器 {root}"))?;
    let status = resp.status();
    let mut out = ProfileCache {
        fetched_at: now_ms(),
        ..Default::default()
    };
    // 204 / 404：角色不存在或没有设置皮肤
    if status.as_u16() == 204 || status.as_u16() == 404 {
        return Ok(out);
    }
    if !status.is_success() {
        bail!("认证服务器返回错误（{}）", status.as_u16());
    }
    let v: Value = resp.json().await.context("角色信息格式错误")?;
    let encoded: Option<String> = v
        .get("properties")
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|p| p.get("name").and_then(|n| n.as_str()) == Some("textures"))
        })
        .and_then(|p| p.get("value"))
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string());
    let Some(encoded) = encoded else {
        return Ok(out);
    };
    let raw = match STANDARD.decode(encoded.as_bytes()) {
        Ok(b) => b,
        Err(_) => STANDARD_NO_PAD
            .decode(encoded.trim_end_matches('=').as_bytes())
            .context("皮肤信息解码失败")?,
    };
    let tex: Value = serde_json::from_slice(&raw).context("皮肤信息格式错误")?;
    let textures = tex.get("textures");
    if let Some(skin) = textures.and_then(|t| t.get("SKIN")) {
        out.skin_url = skin.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
        out.slim = skin
            .get("metadata")
            .and_then(|m| m.get("model"))
            .and_then(|m| m.as_str())
            == Some("slim");
    }
    out.cape_url = textures
        .and_then(|t| t.get("CAPE"))
        .and_then(|c| c.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string());
    Ok(out)
}

fn is_png(b: &[u8]) -> bool {
    b.len() > 8 && b.starts_with(b"\x89PNG\r\n\x1a\n")
}

/// 读取（必要时下载）贴图，返回 data URL。贴图地址通常带内容哈希，按地址缓存即可
async fn texture_data_url(http: &reqwest::Client, dir: &Path, url: &str) -> anyhow::Result<String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        bail!("不支持的贴图地址：{url}");
    }
    let file = dir.join(format!("{}.png", sha1_hex(url.as_bytes())));
    let cached = tokio::fs::read(&file).await.ok().filter(|b| is_png(b));
    let bytes = match cached {
        Some(b) => b,
        None => {
            let resp = http
                .get(url)
                .timeout(Duration::from_secs(20))
                .send()
                .await
                .context("下载皮肤失败")?
                .error_for_status()
                .context("下载皮肤失败")?;
            if resp.content_length().map(|n| n > MAX_TEXTURE as u64).unwrap_or(false) {
                bail!("皮肤文件过大");
            }
            let body = resp.bytes().await.context("下载皮肤失败")?.to_vec();
            if body.len() > MAX_TEXTURE || !is_png(&body) {
                bail!("皮肤不是有效的 PNG 图片");
            }
            tokio::fs::create_dir_all(dir).await?;
            let tmp = file.with_extension("png.tmp");
            tokio::fs::write(&tmp, &body).await?;
            if tokio::fs::rename(&tmp, &file).await.is_err() {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
            body
        }
    };
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}
