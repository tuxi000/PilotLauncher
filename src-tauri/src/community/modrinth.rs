//! Modrinth API v2：https://docs.modrinth.com/api/
//! 镜像：MCIM（api.modrinth.com → mod.mcimirror.top/modrinth，cdn.modrinth.com → mod.mcimirror.top）。

use std::collections::HashSet;

use anyhow::bail;
use serde_json::{json, Value};

use super::{
    get, opt, os, post, s, strs, with_query, Category, Dependency, GalleryImage, Link, Platform, ProjectDetail,
    ResourceFile, ResourceKind, ResourceSummary, ResourceVersion, SearchPage, SearchQuery,
};
use crate::state::Core;

pub const API_BASE: &str = "https://api.modrinth.com/v2";

/// Modrinth 的 project_type 取值（数据包为 datapack，搜索时按 all_project_types 匹配）
pub fn project_type(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Mod => "mod",
        ResourceKind::Modpack => "modpack",
        ResourceKind::Datapack => "datapack",
        ResourceKind::Resourcepack => "resourcepack",
        ResourceKind::Shaderpack => "shader",
        // Modrinth 没有地图分类（地图只从 CurseForge 获取）
        ResourceKind::World => "mod",
    }
}

fn url_type(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Mod => "mod",
        ResourceKind::Modpack => "modpack",
        ResourceKind::Datapack => "datapack",
        ResourceKind::Resourcepack => "resourcepack",
        ResourceKind::Shaderpack => "shader",
        ResourceKind::World => "mod",
    }
}

fn hit_to_summary(h: &Value, kind: ResourceKind) -> ResourceSummary {
    let id = os(h, "project_id").or_else(|| os(h, "id")).unwrap_or_default();
    let slug = s(h, "slug");
    let mut cats = strs(h, "display_categories");
    if cats.is_empty() {
        cats = strs(h, "categories");
    }
    let mut versions = strs(h, "versions");
    if versions.is_empty() {
        versions = strs(h, "game_versions");
    }
    ResourceSummary {
        platform: Platform::Modrinth,
        page_url: format!("https://modrinth.com/{}/{}", url_type(kind), if slug.is_empty() { &id } else { &slug }),
        id,
        slug,
        title: s(h, "title"),
        description: s(h, "description"),
        icon_url: os(h, "icon_url"),
        downloads: h.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
        follows: h.get("follows").or_else(|| h.get("followers")).and_then(|x| x.as_u64()).unwrap_or(0),
        kind,
        author: s(h, "author"),
        categories: cats,
        // 最新版本在后：倒序便于显示
        mc_versions: versions.into_iter().rev().collect(),
        updated: os(h, "date_modified").or_else(|| os(h, "updated")),
    }
}

pub async fn search(core: &Core, q: &SearchQuery) -> anyhow::Result<SearchPage> {
    if q.kind == ResourceKind::World {
        bail!("Modrinth 没有地图分类");
    }
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", project_type(q.kind))]];
    if let Some(v) = opt(&q.mc_version) {
        facets.push(vec![format!("versions:{v}")]);
    }
    if matches!(q.kind, ResourceKind::Mod | ResourceKind::Modpack) {
        if let Some(l) = opt(&q.loader) {
            facets.push(vec![format!("categories:{l}")]);
        }
    }
    if let Some(c) = opt(&q.category) {
        facets.push(vec![format!("categories:{c}")]);
    }
    let index = match q.sort.as_str() {
        "downloads" => "downloads",
        "updated" => "updated",
        "newest" => "newest",
        "follows" => "follows",
        _ => "relevance",
    };
    let size = q.size();
    let url = with_query(
        &format!("{API_BASE}/search"),
        &[
            ("query", q.keyword.trim().to_string()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", index.to_string()),
            ("offset", (q.page * size).to_string()),
            ("limit", size.to_string()),
        ],
    )?;
    let v = get(core, &url).await?;
    let hits: Vec<ResourceSummary> = v
        .get("hits")
        .and_then(|h| h.as_array())
        .map(|a| a.iter().map(|h| hit_to_summary(h, q.kind)).collect())
        .unwrap_or_default();
    let total = v.get("total_hits").and_then(|x| x.as_u64()).unwrap_or(0);
    let has_more = (((q.page + 1) * size) as u64) < total && !hits.is_empty();
    Ok(SearchPage { hits, total, page: q.page, has_more, notices: Vec::new() })
}

pub async fn categories(core: &Core, kind: ResourceKind) -> anyhow::Result<Vec<Category>> {
    let v = get(core, &format!("{API_BASE}/tag/category")).await?;
    let pt = project_type(kind);
    let mut out: Vec<Category> = v
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        // 数据包在 Modrinth 上使用 mod 的分类
        .filter(|c| {
            let t = s(c, "project_type");
            t == pt || (kind == ResourceKind::Datapack && t == "mod")
        })
        .filter(|c| s(c, "header") != "resolutions" || kind == ResourceKind::Resourcepack)
        .map(|c| Category { id: s(c, "name"), name: s(c, "name"), icon: None })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.id == b.id);
    Ok(out)
}

pub async fn project(core: &Core, id: &str, kind: ResourceKind) -> anyhow::Result<ProjectDetail> {
    let v = get(core, &format!("{API_BASE}/project/{id}")).await?;
    let mut summary = hit_to_summary(&v, kind);
    if summary.id.is_empty() {
        summary.id = id.to_string();
    }
    let mut gallery: Vec<(bool, GalleryImage)> = v
        .get("gallery")
        .and_then(|g| g.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some((
                        g.get("featured").and_then(|f| f.as_bool()).unwrap_or(false),
                        GalleryImage { url: os(g, "url")?, title: os(g, "title") },
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    gallery.sort_by_key(|(f, _)| !*f);
    let mut links = vec![Link { label: "Modrinth 页面".into(), url: summary.page_url.clone() }];
    for (k, label) in [("source_url", "源代码"), ("issues_url", "问题反馈"), ("wiki_url", "Wiki"), ("discord_url", "Discord")] {
        if let Some(u) = os(&v, k) {
            links.push(Link { label: label.into(), url: u });
        }
    }
    Ok(ProjectDetail {
        body: s(&v, "body"),
        body_format: "markdown".into(),
        gallery: gallery.into_iter().map(|(_, g)| g).collect(),
        links,
        game_versions: strs(&v, "game_versions").into_iter().rev().collect(),
        loaders: strs(&v, "loaders"),
        summary,
    })
}

pub fn map_version(v: &Value) -> ResourceVersion {
    let files = v
        .get("files")
        .and_then(|f| f.as_array())
        .map(|a| {
            a.iter()
                .map(|f| ResourceFile {
                    url: os(f, "url"),
                    filename: s(f, "filename"),
                    size: f.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
                    sha1: f.pointer("/hashes/sha1").and_then(|x| x.as_str()).map(|x| x.to_string()),
                    sha512: f.pointer("/hashes/sha512").and_then(|x| x.as_str()).map(|x| x.to_string()),
                    primary: f.get("primary").and_then(|x| x.as_bool()).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let dependencies = v
        .get("dependencies")
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let project_id = os(d, "project_id").unwrap_or_default();
                    let version_id = os(d, "version_id");
                    if project_id.is_empty() && version_id.is_none() {
                        return None;
                    }
                    Some(Dependency { project_id, version_id, kind: s(d, "dependency_type") })
                })
                .collect()
        })
        .unwrap_or_default();
    ResourceVersion {
        platform: Platform::Modrinth,
        project_id: s(v, "project_id"),
        id: s(v, "id"),
        name: s(v, "name"),
        version_number: s(v, "version_number"),
        game_versions: strs(v, "game_versions"),
        loaders: strs(v, "loaders"),
        release_type: os(v, "version_type").unwrap_or_else(|| "release".into()),
        date: s(v, "date_published"),
        downloads: v.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
        files,
        dependencies,
    }
}

pub async fn versions(
    core: &Core,
    project_id: &str,
    mc: Option<&str>,
    loaders: &[String],
) -> anyhow::Result<Vec<ResourceVersion>> {
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if !loaders.is_empty() {
        pairs.push(("loaders", serde_json::to_string(loaders)?));
    }
    if let Some(m) = mc.filter(|m| !m.is_empty()) {
        pairs.push(("game_versions", serde_json::to_string(&[m])?));
    }
    let url = with_query(&format!("{API_BASE}/project/{project_id}/version"), &pairs)?;
    let v = get(core, &url).await?;
    Ok(v.as_array().map(|a| a.iter().map(map_version).collect()).unwrap_or_default())
}

pub async fn version(core: &Core, id: &str) -> anyhow::Result<ResourceVersion> {
    let v = get(core, &format!("{API_BASE}/version/{id}")).await?;
    Ok(map_version(&v))
}

pub async fn brief(core: &Core, id: &str) -> anyhow::Result<(String, Option<String>)> {
    let v = get(core, &format!("{API_BASE}/project/{id}")).await?;
    Ok((s(&v, "title"), os(&v, "icon_url")))
}

/// 根据文件 sha1 反查所属项目（POST /version_files）
pub async fn projects_by_hashes(core: &Core, hashes: &[String]) -> anyhow::Result<HashSet<String>> {
    let v = post(core, &format!("{API_BASE}/version_files"), &json!({ "hashes": hashes, "algorithm": "sha1" })).await?;
    Ok(v.as_object()
        .map(|o| o.values().filter_map(|x| os(x, "project_id")).collect())
        .unwrap_or_default())
}
