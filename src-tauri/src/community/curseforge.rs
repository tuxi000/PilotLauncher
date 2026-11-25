//! CurseForge API v1（https://docs.curseforge.com/rest-api/）。
//!
//! API Key 只从本地配置文件 config.local.json 读取（已加入 .gitignore），源码中不包含任何真实 Key；
//! 请求官方 API 时带 `x-api-key`，请求 MCIM 镜像时不带（镜像不需要，也避免泄露 Key）。

use std::collections::HashMap;

use anyhow::{anyhow, bail};
use serde_json::{json, Value};

use super::{
    get, has_cf_key, opt, os, post, s, with_query, Category, Dependency, GalleryImage, Link, Platform, ProjectDetail,
    ResourceFile, ResourceKind, ResourceSummary, ResourceVersion, SearchPage, SearchQuery,
};
use crate::settings::LocalConfig;
use crate::state::Core;

pub const API_BASE: &str = "https://api.curseforge.com/v1";
/// Minecraft 的 gameId
pub const GAME_ID: u32 = 432;
/// CurseForge 限制 index + pageSize ≤ 10000
const MAX_RESULTS: u32 = 10_000;

/// 读取 CurseForge API Key；未配置时返回带指引的错误
pub fn api_key(cfg: &LocalConfig) -> anyhow::Result<String> {
    cfg.curseforge_api_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            anyhow!(
                "未配置 CurseForge API Key：请复制 config.local.example.json 为 config.local.json，\
                 放在启动器 exe 同目录或数据目录，并填写 curseforgeApiKey"
            )
        })
}

fn ensure_key(core: &Core) -> anyhow::Result<()> {
    if !has_cf_key(core) {
        api_key(&core.local_config)?;
    }
    Ok(())
}

/// CurseForge 资源分类 id（classId）
pub fn class_id(kind: ResourceKind) -> u32 {
    match kind {
        ResourceKind::Mod => 6,
        ResourceKind::Modpack => 4471,
        ResourceKind::Resourcepack => 12,
        ResourceKind::World => 17,
        ResourceKind::Shaderpack => 6552,
        ResourceKind::Datapack => 6945,
    }
}

pub fn kind_of_class(class: u64) -> Option<ResourceKind> {
    Some(match class {
        6 => ResourceKind::Mod,
        4471 => ResourceKind::Modpack,
        12 => ResourceKind::Resourcepack,
        17 => ResourceKind::World,
        6552 => ResourceKind::Shaderpack,
        6945 => ResourceKind::Datapack,
        _ => return None,
    })
}

/// modLoaderType：1 Forge、4 Fabric、5 Quilt、6 NeoForge
pub fn loader_type(name: &str) -> Option<u32> {
    match name.to_ascii_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn data(v: &Value) -> Value {
    v.get("data").cloned().unwrap_or(Value::Null)
}

pub fn mod_to_summary(m: &Value, fallback: ResourceKind) -> ResourceSummary {
    let id = m.get("id").and_then(|x| x.as_u64()).map(|x| x.to_string()).unwrap_or_default();
    let kind = m
        .get("classId")
        .and_then(|c| c.as_u64())
        .and_then(kind_of_class)
        .unwrap_or(fallback);
    let mut mc_versions: Vec<String> = Vec::new();
    if let Some(a) = m.get("latestFilesIndexes").and_then(|x| x.as_array()) {
        for f in a {
            let gv = s(f, "gameVersion");
            if !gv.is_empty() && !mc_versions.contains(&gv) {
                mc_versions.push(gv);
            }
        }
    }
    let slug = s(m, "slug");
    ResourceSummary {
        platform: Platform::Curseforge,
        page_url: m
            .pointer("/links/websiteUrl")
            .and_then(|x| x.as_str())
            .map(|x| x.to_string())
            .unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/search?search={slug}")),
        id,
        slug,
        title: s(m, "name"),
        description: s(m, "summary"),
        icon_url: m.pointer("/logo/thumbnailUrl").and_then(|x| x.as_str()).map(|x| x.to_string()),
        downloads: m.get("downloadCount").and_then(|x| x.as_f64()).unwrap_or(0.0) as u64,
        follows: m.get("thumbsUpCount").and_then(|x| x.as_u64()).unwrap_or(0),
        kind,
        author: m
            .get("authors")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .map(|a| s(a, "name"))
            .unwrap_or_default(),
        categories: m
            .get("categories")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().map(|c| s(c, "name")).filter(|n| !n.is_empty()).collect())
            .unwrap_or_default(),
        mc_versions,
        updated: os(m, "dateModified"),
    }
}

pub async fn search(core: &Core, q: &SearchQuery) -> anyhow::Result<SearchPage> {
    ensure_key(core)?;
    let size = q.size();
    let index = q.page * size;
    if index + size > MAX_RESULTS {
        return Ok(SearchPage { hits: Vec::new(), total: MAX_RESULTS as u64, page: q.page, has_more: false, notices: Vec::new() });
    }
    // sortField：2 人气、3 最近更新、6 下载量、11 发布时间、1 精选
    let sort_field = match q.sort.as_str() {
        "downloads" => 6,
        "updated" => 3,
        "newest" => 11,
        "follows" => 2,
        _ => {
            if q.keyword.trim().is_empty() {
                2
            } else {
                1
            }
        }
    };
    let mut pairs: Vec<(&str, String)> = vec![
        ("gameId", GAME_ID.to_string()),
        ("classId", class_id(q.kind).to_string()),
        ("sortField", sort_field.to_string()),
        ("sortOrder", "desc".into()),
        ("index", index.to_string()),
        ("pageSize", size.to_string()),
    ];
    if !q.keyword.trim().is_empty() {
        pairs.push(("searchFilter", q.keyword.trim().to_string()));
    }
    if let Some(v) = opt(&q.mc_version) {
        pairs.push(("gameVersion", v.to_string()));
    }
    if matches!(q.kind, ResourceKind::Mod | ResourceKind::Modpack) {
        if let Some(t) = opt(&q.loader).and_then(loader_type) {
            pairs.push(("modLoaderType", t.to_string()));
        }
    }
    if let Some(c) = opt(&q.category).filter(|c| c.chars().all(|ch| ch.is_ascii_digit())) {
        pairs.push(("categoryId", c.to_string()));
    }
    let url = with_query(&format!("{API_BASE}/mods/search"), &pairs)?;
    let v = get(core, &url).await?;
    let hits: Vec<ResourceSummary> = v
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| a.iter().map(|m| mod_to_summary(m, q.kind)).collect())
        .unwrap_or_default();
    let total = v.pointer("/pagination/totalCount").and_then(|x| x.as_u64()).unwrap_or(0).min(MAX_RESULTS as u64);
    let has_more = !hits.is_empty() && ((index + size) as u64) < total;
    Ok(SearchPage { hits, total, page: q.page, has_more, notices: Vec::new() })
}

pub async fn categories(core: &Core, kind: ResourceKind) -> anyhow::Result<Vec<Category>> {
    ensure_key(core)?;
    let url = format!("{API_BASE}/categories?gameId={GAME_ID}&classId={}", class_id(kind));
    let v = get(core, &url).await?;
    let mut out: Vec<Category> = data(&v)
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|c| c.get("isClass").and_then(|x| x.as_bool()) != Some(true))
        .filter_map(|c| {
            Some(Category {
                id: c.get("id")?.as_u64()?.to_string(),
                name: s(c, "name"),
                icon: os(c, "iconUrl"),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub async fn project(core: &Core, id: &str, kind: ResourceKind) -> anyhow::Result<ProjectDetail> {
    ensure_key(core)?;
    let (mod_url, desc_url) = (format!("{API_BASE}/mods/{id}"), format!("{API_BASE}/mods/{id}/description"));
    let (m, desc) = tokio::join!(get(core, &mod_url), get(core, &desc_url));
    let m = data(&m?);
    let body = desc.ok().and_then(|d| d.get("data").and_then(|x| x.as_str()).map(|x| x.to_string())).unwrap_or_default();
    let summary = mod_to_summary(&m, kind);
    let gallery = m
        .get("screenshots")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|g| Some(GalleryImage { url: os(g, "url")?, title: os(g, "title") }))
                .collect()
        })
        .unwrap_or_default();
    let mut links = vec![Link { label: "CurseForge 页面".into(), url: summary.page_url.clone() }];
    for (k, label) in [("sourceUrl", "源代码"), ("issuesUrl", "问题反馈"), ("wikiUrl", "Wiki")] {
        if let Some(u) = m.pointer(&format!("/links/{k}")).and_then(|x| x.as_str()).filter(|x| !x.is_empty()) {
            links.push(Link { label: label.into(), url: u.to_string() });
        }
    }
    let mut loaders = Vec::new();
    if let Some(a) = m.get("latestFilesIndexes").and_then(|x| x.as_array()) {
        for f in a {
            if let Some(t) = f.get("modLoader").and_then(|x| x.as_u64()) {
                let n = match t {
                    1 => "forge",
                    4 => "fabric",
                    5 => "quilt",
                    6 => "neoforge",
                    _ => continue,
                };
                if !loaders.iter().any(|l: &String| l == n) {
                    loaders.push(n.to_string());
                }
            }
        }
    }
    Ok(ProjectDetail {
        game_versions: summary.mc_versions.clone(),
        summary,
        body,
        body_format: "html".into(),
        gallery,
        links,
        loaders,
    })
}

pub fn map_file(f: &Value) -> ResourceVersion {
    let mut game_versions = Vec::new();
    let mut loaders = Vec::new();
    for gv in f.get("gameVersions").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
        let Some(g) = gv.as_str() else { continue };
        let lower = g.to_ascii_lowercase();
        if ["forge", "fabric", "quilt", "neoforge"].contains(&lower.as_str()) {
            loaders.push(lower);
        } else if g.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            game_versions.push(g.to_string());
        }
    }
    let sha1 = f
        .get("hashes")
        .and_then(|h| h.as_array())
        .and_then(|a| a.iter().find(|h| h.get("algo").and_then(|x| x.as_u64()) == Some(1)))
        .map(|h| s(h, "value"))
        .filter(|x| !x.is_empty());
    let dependencies = f
        .get("dependencies")
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let pid = d.get("modId")?.as_u64()?.to_string();
                    let kind = match d.get("relationType").and_then(|x| x.as_u64()).unwrap_or(0) {
                        3 => "required",
                        2 => "optional",
                        5 => "incompatible",
                        1 | 6 => "embedded",
                        _ => "tool",
                    };
                    Some(Dependency { project_id: pid, version_id: None, kind: kind.into() })
                })
                .collect()
        })
        .unwrap_or_default();
    let release_type = match f.get("releaseType").and_then(|x| x.as_u64()).unwrap_or(1) {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    };
    let id = f.get("id").and_then(|x| x.as_u64()).map(|x| x.to_string()).unwrap_or_default();
    let filename = s(f, "fileName");
    ResourceVersion {
        platform: Platform::Curseforge,
        project_id: f.get("modId").and_then(|x| x.as_u64()).map(|x| x.to_string()).unwrap_or_default(),
        id,
        name: os(f, "displayName").unwrap_or_else(|| filename.clone()),
        version_number: filename.clone(),
        game_versions,
        loaders,
        release_type: release_type.into(),
        date: s(f, "fileDate"),
        downloads: f.get("downloadCount").and_then(|x| x.as_u64()).unwrap_or(0),
        files: vec![ResourceFile {
            url: os(f, "downloadUrl"),
            filename,
            size: f.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0),
            sha1,
            sha512: None,
            primary: true,
        }],
        dependencies,
    }
}

pub async fn versions(
    core: &Core,
    project_id: &str,
    mc: Option<&str>,
    loaders: &[String],
) -> anyhow::Result<Vec<ResourceVersion>> {
    ensure_key(core)?;
    let mut pairs: Vec<(&str, String)> = vec![("pageSize", "50".into()), ("index", "0".into())];
    if let Some(m) = mc.filter(|m| !m.is_empty()) {
        pairs.push(("gameVersion", m.to_string()));
    }
    // 只有一个加载器时交给服务端过滤；Quilt 实例（quilt + fabric）在本地过滤
    if loaders.len() == 1 {
        if let Some(t) = loader_type(&loaders[0]) {
            pairs.push(("modLoaderType", t.to_string()));
        }
    }
    let url = with_query(&format!("{API_BASE}/mods/{project_id}/files"), &pairs)?;
    let v = get(core, &url).await?;
    let mut list: Vec<ResourceVersion> = data(&v)
        .as_array()
        .map(|a| a.iter().map(map_file).collect())
        .unwrap_or_default();
    if loaders.len() > 1 {
        list.retain(|f| f.loaders.is_empty() || f.loaders.iter().any(|l| loaders.contains(l)));
    }
    list.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(list)
}

pub async fn file(core: &Core, project_id: &str, file_id: &str) -> anyhow::Result<ResourceVersion> {
    let v = get(core, &format!("{API_BASE}/mods/{project_id}/files/{file_id}")).await?;
    Ok(map_file(&data(&v)))
}

pub async fn brief(core: &Core, id: &str) -> anyhow::Result<(String, Option<String>)> {
    let v = get(core, &format!("{API_BASE}/mods/{id}")).await?;
    let m = data(&v);
    Ok((s(&m, "name"), m.pointer("/logo/thumbnailUrl").and_then(|x| x.as_str()).map(|x| x.to_string())))
}

/// 批量获取文件信息（整合包使用），每批最多 500 个
pub async fn files_batch(core: &Core, ids: &[u64]) -> anyhow::Result<Vec<Value>> {
    ensure_key(core)?;
    let mut out = Vec::new();
    for chunk in ids.chunks(500) {
        let v = post(core, &format!("{API_BASE}/mods/files"), &json!({ "fileIds": chunk })).await?;
        out.extend(data(&v).as_array().cloned().unwrap_or_default());
    }
    Ok(out)
}

/// 批量获取项目信息：modId → (名称, 项目页, classId)
pub async fn mods_batch(core: &Core, ids: &[u64]) -> anyhow::Result<HashMap<u64, (String, String, u64)>> {
    ensure_key(core)?;
    let mut out = HashMap::new();
    for chunk in ids.chunks(500) {
        let v = post(core, &format!("{API_BASE}/mods"), &json!({ "modIds": chunk })).await?;
        for m in data(&v).as_array().cloned().unwrap_or_default() {
            let Some(id) = m.get("id").and_then(|x| x.as_u64()) else { continue };
            let url = m.pointer("/links/websiteUrl").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let class = m.get("classId").and_then(|x| x.as_u64()).unwrap_or(6);
            out.insert(id, (s(&m, "name"), url, class));
        }
    }
    if out.is_empty() && !ids.is_empty() {
        bail!("获取 CurseForge 项目信息失败");
    }
    Ok(out)
}
