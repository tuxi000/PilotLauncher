//! Fabric / Quilt：从 meta 服务获取 profile JSON 并合并进实例 JSON。
//!
//! - Fabric：meta.fabricmc.net/v2（BMCLAPI 镜像 /fabric-meta，由 download::source 自动替换）
//! - Quilt：meta.quiltmc.org/v3（BMCLAPI 没有 Quilt meta 镜像，只走官方；库文件 maven 仍可走镜像）
//!
//! 版本列表：Fabric / Quilt 的加载器版本与游戏版本无关，因此改为请求两个小接口并缓存（所有游戏版本共用）：
//!   /versions/loader（全部加载器版本） + /versions/game（支持的游戏版本），
//! 不再每换一个游戏版本就下载一次带完整 launcherMeta 的 /versions/loader/<mc>（几百 KB）。
//! 这两个接口失败时回退到按版本的接口。

use std::sync::Arc;

use anyhow::{bail, Context};
use serde_json::Value;

use super::{merge_json, meta, LoaderKind, LoaderVersion, Stager};
use crate::download::fetch_with;
use crate::state::Core;
use crate::tasks::TaskHandle;

pub const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
pub const QUILT_META: &str = "https://meta.quiltmc.org/v3";

fn base(kind: LoaderKind) -> &'static str {
    if kind == LoaderKind::Quilt {
        QUILT_META
    } else {
        FABRIC_META
    }
}

fn seg(s: &str) -> String {
    s.replace(' ', "%20")
}

fn parse_loaders(kind: LoaderKind, mc: &str, items: &[Value]) -> Vec<LoaderVersion> {
    let mut out = Vec::new();
    for item in items {
        // 按版本的接口每项为 {loader:{...}, intermediary:{...}, ...}，全量接口每项就是 loader 本身
        let loader = item.get("loader").unwrap_or(item);
        let Some(version) = loader.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        let lower = version.to_ascii_lowercase();
        let stable = loader
            .get("stable")
            .and_then(|s| s.as_bool())
            .unwrap_or(!(lower.contains("beta") || lower.contains("pre") || lower.contains("rc") || lower.contains("alpha")));
        out.push(LoaderVersion {
            kind,
            id: version.to_string(),
            version: version.to_string(),
            mc_version: mc.to_string(),
            stable,
            recommended: false,
            latest: false,
            note: None,
        });
    }
    // meta 已按新到旧排序；这里保持原顺序
    out
}

async fn list_by_mc(core: &Core, kind: LoaderKind, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let url = format!("{}/versions/loader/{}", base(kind), seg(mc));
    let key = format!("{}-loader-{mc}", kind.id());
    let data = meta::cached(core, &key, force, || meta::fetch_meta(core, &url, meta::is_json_array)).await?;
    let items: Vec<Value> = serde_json::from_slice(data.as_slice())?;
    Ok(parse_loaders(kind, mc, &items))
}

pub async fn list(core: &Core, kind: LoaderKind, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let games_url = format!("{}/versions/game", base(kind));
    let loaders_url = format!("{}/versions/loader", base(kind));
    let games_key = format!("{}-games", kind.id());
    let loaders_key = format!("{}-loaders", kind.id());
    let (games, loaders) = tokio::join!(
        meta::cached(core, &games_key, force, || meta::fetch_meta(core, &games_url, meta::is_json_array)),
        meta::cached(core, &loaders_key, force, || meta::fetch_meta(core, &loaders_url, meta::is_json_array))
    );
    if let (Ok(g), Ok(l)) = (&games, &loaders) {
        let games: Vec<Value> = serde_json::from_slice(g.as_slice()).unwrap_or_default();
        let items: Vec<Value> = serde_json::from_slice(l.as_slice()).unwrap_or_default();
        if !games.is_empty() && !items.is_empty() {
            let supported = games.iter().any(|x| x.get("version").and_then(|v| v.as_str()) == Some(mc));
            if !supported {
                // 由 list_versions 给出“暂不支持”的提示
                return Ok(Vec::new());
            }
            return Ok(parse_loaders(kind, mc, &items));
        }
    }
    list_by_mc(core, kind, mc, force)
        .await
        .with_context(|| format!("获取 {} 版本列表失败", kind.display_name()))
}

pub async fn install(
    core: &Arc<Core>,
    task: &Arc<TaskHandle>,
    st: &mut Stager,
    kind: LoaderKind,
    mc: &str,
    loader: &str,
    json: &mut Value,
) -> anyhow::Result<()> {
    st.next(format!("获取 {} {loader} 配置", kind.display_name()), 1, 0);
    let url = format!("{}/versions/loader/{}/{}/profile/json", base(kind), seg(mc), seg(loader));
    let profile: Value = fetch_with(&core.http, &core.sources(&url), core.settings().retries, |b| {
        let v: Value = serde_json::from_slice(b)?;
        if v.get("mainClass").is_none() || v.get("libraries").is_none() {
            bail!("profile JSON 格式不正确");
        }
        Ok(v)
    })
    .await
    .with_context(|| format!("获取 {} profile 失败", kind.display_name()))?;
    *json = merge_json(json, &profile);
    task.add_progress(1, 0);
    Ok(())
}
