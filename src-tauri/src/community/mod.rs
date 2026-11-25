//! 社区资源：Modrinth、CurseForge 与 MCIM 镜像。
//!
//! - 搜索 / 详情 / 版本列表 / 依赖解析：各平台在 modrinth.rs、curseforge.rs 中映射为统一结构
//! - 所有 API 请求与文件下载都遵守「社区资源源策略」（官方优先 / 镜像优先 / 只用官方），镜像为 MCIM
//! - 安装到实例：mod / 资源包 / 光影包 / 数据包（指定存档）/ 地图（解压到 saves）
//! - 已安装记录：实例目录下的 glass-resources.json（用于依赖去重、更新时替换旧文件）

pub mod curseforge;
pub mod local;
pub mod mcim;
pub mod modpack;
pub mod modrinth;
pub mod world;

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::download::{download_all, total_size, DownloadItem, DownloadOptions, Verify};
use crate::instance::{self, InstanceMeta};
use crate::state::Core;
use crate::tasks::{spawn_task, TaskHandle};
use crate::util::{now_ms, read_json, write_json_atomic};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Mod,
    Modpack,
    Datapack,
    Resourcepack,
    Shaderpack,
    World,
}

impl ResourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            ResourceKind::Mod => "模组",
            ResourceKind::Modpack => "整合包",
            ResourceKind::Datapack => "数据包",
            ResourceKind::Resourcepack => "资源包",
            ResourceKind::Shaderpack => "光影包",
            ResourceKind::World => "地图",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Modrinth,
    Curseforge,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchPlatform {
    Modrinth,
    Curseforge,
    All,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub platform: SearchPlatform,
    pub kind: ResourceKind,
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub mc_version: Option<String>,
    /// fabric / forge / neoforge / quilt
    #[serde(default)]
    pub loader: Option<String>,
    /// Modrinth 为分类名，CurseForge 为分类 id
    #[serde(default)]
    pub category: Option<String>,
    /// relevance / downloads / updated / newest / follows
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub page_size: u32,
}

impl SearchQuery {
    pub fn size(&self) -> u32 {
        if self.page_size == 0 {
            20
        } else {
            self.page_size.clamp(1, 50)
        }
    }
}

/// 去掉空字符串
pub fn opt(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(|x| x.trim()).filter(|x| !x.is_empty())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSummary {
    pub platform: Platform,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub kind: ResourceKind,
    pub author: String,
    pub categories: Vec<String>,
    pub mc_versions: Vec<String>,
    pub updated: Option<String>,
    pub page_url: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub hits: Vec<ResourceSummary>,
    pub total: u64,
    pub page: u32,
    pub has_more: bool,
    /// 部分平台失败或不可用时的提示
    pub notices: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct GalleryImage {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Link {
    pub label: String,
    pub url: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub summary: ResourceSummary,
    /// Modrinth 为 markdown，CurseForge 为 HTML（前端负责渲染与净化）
    pub body: String,
    pub body_format: String,
    pub gallery: Vec<GalleryImage>,
    pub links: Vec<Link>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResourceFile {
    /// CurseForge 作者禁止第三方下载时为 None
    pub url: Option<String>,
    pub filename: String,
    pub size: u64,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub primary: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    /// Modrinth 可能只给出 version_id，此时为空字符串
    pub project_id: String,
    pub version_id: Option<String>,
    /// required / optional / incompatible / embedded
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResourceVersion {
    pub platform: Platform,
    pub project_id: String,
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// release / beta / alpha
    pub release_type: String,
    pub date: String,
    pub downloads: u64,
    pub files: Vec<ResourceFile>,
    pub dependencies: Vec<Dependency>,
}

impl ResourceVersion {
    pub fn primary_file(&self) -> Option<&ResourceFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub platform: Platform,
    pub project_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version: Option<ResourceVersion>,
    /// 用户选择的资源本身（其余为依赖）
    pub root: bool,
    pub installed: bool,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallItem {
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub version: ResourceVersion,
}

/// 资源安装到实例中的目标目录（版本隔离：全部在实例目录内）。
/// 数据包需要指定存档；地图解压到 saves/。
pub fn target_dir(instance_dir: &Path, kind: ResourceKind, save: Option<&str>) -> anyhow::Result<PathBuf> {
    Ok(match kind {
        ResourceKind::Mod => instance_dir.join("mods"),
        ResourceKind::Resourcepack => instance_dir.join("resourcepacks"),
        ResourceKind::Shaderpack => instance_dir.join("shaderpacks"),
        ResourceKind::World => instance_dir.join("saves"),
        ResourceKind::Datapack => match save {
            Some(s) if !s.is_empty() && !s.contains('/') && !s.contains('\\') && s != ".." => {
                instance_dir.join("saves").join(s).join("datapacks")
            }
            _ => bail!("安装数据包需要先选择一个存档"),
        },
        ResourceKind::Modpack => bail!("整合包会创建新的实例，不安装到现有实例"),
    })
}

/// 校验并清理文件名（防止路径穿越）
pub fn safe_file_name(name: &str) -> anyhow::Result<String> {
    let n = name.trim();
    const ILLEGAL: &str = "\\/:*?\"<>|";
    if n.is_empty() || n == "." || n == ".." || n.contains("..") || n.chars().any(|c| ILLEGAL.contains(c) || c.is_control()) {
        bail!("非法的文件名：{name}");
    }
    Ok(n.to_string())
}

// ───────────────────────── HTTP（遵守社区源策略） ─────────────────────────

pub fn urls(core: &Core, url: &str) -> Vec<String> {
    mcim::candidates(core.settings().community_source, url)
}

pub fn has_cf_key(core: &Core) -> bool {
    core.local_config.curseforge_api_key.as_deref().map(|k| !k.trim().is_empty()).unwrap_or(false)
}

async fn api(core: &Core, method: reqwest::Method, url: &str, body: Option<&Value>) -> anyhow::Result<Value> {
    let key = core.local_config.curseforge_api_key.clone();
    let retries = core.settings().retries.min(2);
    let candidates = urls(core, url);
    let mut last: Option<anyhow::Error> = None;
    for attempt in 0..=retries {
        let mut all_404 = true;
        for u in &candidates {
            let mut req = core
                .http
                .request(method.clone(), u.as_str())
                .timeout(Duration::from_secs(25))
                .header("Accept", "application/json");
            // 只把 Key 发给 CurseForge 官方，不发给镜像
            if u.starts_with(curseforge::API_BASE) {
                if let Some(k) = &key {
                    req = req.header("x-api-key", k.as_str());
                }
            }
            if let Some(b) = body {
                req = req.json(b);
            }
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        match resp.json::<Value>().await {
                            Ok(v) => return Ok(v),
                            Err(e) => last = Some(anyhow!("{u}：响应格式错误：{e}")),
                        }
                        all_404 = false;
                    } else if status.as_u16() == 404 {
                        last = Some(anyhow!("资源不存在（404）"));
                    } else {
                        all_404 = false;
                        last = Some(if status.as_u16() == 403 && u.starts_with(curseforge::API_BASE) {
                            anyhow!("CurseForge 拒绝了请求（403），请检查 config.local.json 中的 API Key 是否有效")
                        } else {
                            anyhow!("{u}：HTTP {status}")
                        });
                    }
                }
                Err(e) => {
                    all_404 = false;
                    last = Some(anyhow!("{u}：{e}"));
                }
            }
        }
        if all_404 || attempt == retries {
            break;
        }
        tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
    }
    Err(last.unwrap_or_else(|| anyhow!("没有可用的地址")))
}

pub async fn get(core: &Core, url: &str) -> anyhow::Result<Value> {
    api(core, reqwest::Method::GET, url, None).await
}

pub async fn post(core: &Core, url: &str, body: &Value) -> anyhow::Result<Value> {
    api(core, reqwest::Method::POST, url, Some(body)).await
}

/// 拼接带查询参数的地址（自动编码）
pub fn with_query(base: &str, pairs: &[(&str, String)]) -> anyhow::Result<String> {
    Ok(reqwest::Url::parse_with_params(base, pairs.iter().map(|(k, v)| (*k, v.as_str())))?.to_string())
}

pub fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

pub fn os(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(|x| x.to_string())
}

pub fn strs(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|x| x.to_string()).collect())
        .unwrap_or_default()
}

// ───────────────────────── 搜索 / 详情 / 版本 ─────────────────────────

pub async fn search(core: &Core, q: &SearchQuery) -> anyhow::Result<SearchPage> {
    // Modrinth 没有地图分类：地图只走 CurseForge
    let platform = if q.kind == ResourceKind::World { SearchPlatform::Curseforge } else { q.platform };
    let cf = has_cf_key(core);
    match platform {
        SearchPlatform::Modrinth => modrinth::search(core, q).await,
        SearchPlatform::Curseforge => {
            if !cf {
                bail!("未配置 CurseForge API Key，无法搜索 CurseForge 资源");
            }
            curseforge::search(core, q).await
        }
        SearchPlatform::All => {
            if !cf {
                let mut p = modrinth::search(core, q).await?;
                p.notices.push("未配置 CurseForge API Key，只显示 Modrinth 的结果".into());
                return Ok(p);
            }
            let (a, b) = tokio::join!(modrinth::search(core, q), curseforge::search(core, q));
            let mut notices = Vec::new();
            let mut ok = 0;
            let (ha, ta, ma) = match a {
                Ok(p) => {
                    ok += 1;
                    (p.hits, p.total, p.has_more)
                }
                Err(e) => {
                    notices.push(format!("Modrinth 搜索失败：{e:#}"));
                    (Vec::new(), 0, false)
                }
            };
            let (hb, tb, mb) = match b {
                Ok(p) => {
                    ok += 1;
                    (p.hits, p.total, p.has_more)
                }
                Err(e) => {
                    notices.push(format!("CurseForge 搜索失败：{e:#}"));
                    (Vec::new(), 0, false)
                }
            };
            if ok == 0 {
                bail!("{}", notices.join("\n"));
            }
            let mut hits = Vec::with_capacity(ha.len() + hb.len());
            let (mut ia, mut ib) = (ha.into_iter(), hb.into_iter());
            loop {
                let (x, y) = (ia.next(), ib.next());
                if x.is_none() && y.is_none() {
                    break;
                }
                hits.extend(x);
                hits.extend(y);
            }
            Ok(SearchPage { hits, total: ta + tb, page: q.page, has_more: ma || mb, notices })
        }
    }
}

pub async fn categories(core: &Core, platform: Platform, kind: ResourceKind) -> anyhow::Result<Vec<Category>> {
    match platform {
        Platform::Modrinth => modrinth::categories(core, kind).await,
        Platform::Curseforge => {
            if !has_cf_key(core) {
                return Ok(Vec::new());
            }
            curseforge::categories(core, kind).await
        }
    }
}

pub async fn project(core: &Core, platform: Platform, id: &str, kind: ResourceKind) -> anyhow::Result<ProjectDetail> {
    match platform {
        Platform::Modrinth => modrinth::project(core, id, kind).await,
        Platform::Curseforge => curseforge::project(core, id, kind).await,
    }
}

pub async fn versions(
    core: &Core,
    platform: Platform,
    project_id: &str,
    mc: Option<&str>,
    loaders: &[String],
) -> anyhow::Result<Vec<ResourceVersion>> {
    match platform {
        Platform::Modrinth => modrinth::versions(core, project_id, mc, loaders).await,
        Platform::Curseforge => curseforge::versions(core, project_id, mc, loaders).await,
    }
}

async fn version_by_id(core: &Core, platform: Platform, project_id: &str, id: &str) -> anyhow::Result<ResourceVersion> {
    match platform {
        Platform::Modrinth => modrinth::version(core, id).await,
        Platform::Curseforge => curseforge::file(core, project_id, id).await,
    }
}

async fn brief(core: &Core, platform: Platform, id: &str) -> anyhow::Result<(String, Option<String>)> {
    match platform {
        Platform::Modrinth => modrinth::brief(core, id).await,
        Platform::Curseforge => curseforge::brief(core, id).await,
    }
}

/// 实例可用的 mod 加载器名（Quilt 兼容 Fabric mod）
pub fn instance_loaders(meta: &InstanceMeta) -> Vec<String> {
    match meta.loader.as_ref().map(|l| l.kind.as_str()) {
        Some("fabric") => vec!["fabric".into()],
        Some("quilt") => vec!["quilt".into(), "fabric".into()],
        Some("forge") => vec!["forge".into()],
        Some("neoforge") => vec!["neoforge".into()],
        _ => Vec::new(),
    }
}

/// 为实例挑选最合适的版本：先正式版，再测试版
pub async fn best_version(
    core: &Core,
    platform: Platform,
    project_id: &str,
    mc: &str,
    loaders: &[String],
) -> anyhow::Result<Option<ResourceVersion>> {
    let list = versions(core, platform, project_id, Some(mc), loaders).await?;
    Ok(list
        .iter()
        .find(|v| v.release_type == "release")
        .or_else(|| list.first())
        .cloned())
}

// ───────────────────────── 已安装记录 ─────────────────────────

pub const INDEX_FILE: &str = "glass-resources.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub kind: ResourceKind,
    /// 相对实例目录的路径，如 mods/sodium.jar、saves/World/datapacks/x.zip
    pub path: String,
    pub platform: Platform,
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub installed_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ResourceIndex {
    #[serde(default)]
    pub entries: Vec<IndexEntry>,
}

impl ResourceIndex {
    pub fn load(instance_dir: &Path) -> Self {
        read_json(&instance_dir.join(INDEX_FILE)).unwrap_or_default()
    }

    pub fn save(&self, instance_dir: &Path) -> anyhow::Result<()> {
        write_json_atomic(&instance_dir.join(INDEX_FILE), self)
    }

    /// 文件（或其 .disabled 版本）仍存在的记录
    pub fn alive(&self, instance_dir: &Path) -> Vec<&IndexEntry> {
        self.entries
            .iter()
            .filter(|e| {
                let p = instance_dir.join(&e.path);
                p.exists() || PathBuf::from(format!("{}.disabled", p.display())).exists()
            })
            .collect()
    }

    pub fn find_by_path(&self, rel: &str) -> Option<&IndexEntry> {
        let r = rel.trim_end_matches(".disabled").replace('\\', "/");
        self.entries.iter().find(|e| e.path.eq_ignore_ascii_case(&r))
    }
}

/// 已安装的项目 id（同平台）。Modrinth 的 mod 额外按文件 sha1 反查，能识别手动放入的文件。
async fn installed_projects(core: &Core, instance_dir: &Path, kind: ResourceKind, platform: Platform) -> HashSet<String> {
    let index = ResourceIndex::load(instance_dir);
    let mut set: HashSet<String> = index
        .alive(instance_dir)
        .into_iter()
        .filter(|e| e.kind == kind && e.platform == platform)
        .map(|e| e.project_id.clone())
        .collect();
    if platform == Platform::Modrinth && kind == ResourceKind::Mod {
        let dir = instance_dir.join("mods");
        let mut hashes = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.filter_map(|e| e.ok()).take(400) {
                let p = e.path();
                let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                if p.is_file() && (n.ends_with(".jar") || n.ends_with(".jar.disabled")) {
                    if let Ok(h) = crate::util::file_sha1(&p).await {
                        hashes.push(h);
                    }
                }
            }
        }
        if !hashes.is_empty() {
            if let Ok(found) = modrinth::projects_by_hashes(core, &hashes).await {
                set.extend(found);
            }
        }
    }
    set
}

/// 生成安装计划：用户选择的版本 + 递归的必需依赖（已安装的标记跳过）
pub async fn plan_install(
    core: &Core,
    instance: &str,
    kind: ResourceKind,
    title: String,
    icon_url: Option<String>,
    root: ResourceVersion,
) -> anyhow::Result<Vec<PlanItem>> {
    let dirs = core.game_dirs();
    let meta = instance::load_meta(&dirs, instance)?;
    let inst_dir = dirs.instance_dir(instance);
    let platform = root.platform;
    let installed = installed_projects(core, &inst_dir, kind, platform).await;
    let mut out = vec![PlanItem {
        platform,
        project_id: root.project_id.clone(),
        title,
        icon_url,
        installed: installed.contains(&root.project_id),
        version: Some(root.clone()),
        root: true,
        note: None,
    }];
    if kind != ResourceKind::Mod {
        return Ok(out);
    }
    let loaders = instance_loaders(&meta);
    let mc = meta.mc_version.clone();
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(root.project_id.clone());
    let mut queue: VecDeque<(Dependency, u32)> =
        root.dependencies.iter().filter(|d| d.kind == "required").cloned().map(|d| (d, 1)).collect();
    while let Some((dep, depth)) = queue.pop_front() {
        if out.len() >= 80 {
            break;
        }
        // 只给出 version_id 的依赖先取版本，拿到 project_id
        let mut pinned: Option<ResourceVersion> = None;
        let mut pid = dep.project_id.clone();
        if let Some(vid) = &dep.version_id {
            if let Ok(v) = version_by_id(core, platform, &pid, vid).await {
                if pid.is_empty() {
                    pid = v.project_id.clone();
                }
                pinned = Some(v);
            }
        }
        if pid.is_empty() || !visited.insert(pid.clone()) {
            continue;
        }
        let (dep_title, dep_icon) = brief(core, platform, &pid).await.unwrap_or_else(|_| (pid.clone(), None));
        if installed.contains(&pid) {
            out.push(PlanItem {
                platform,
                project_id: pid,
                title: dep_title,
                icon_url: dep_icon,
                version: None,
                root: false,
                installed: true,
                note: Some("已安装，跳过".into()),
            });
            continue;
        }
        let ver = match pinned {
            Some(v) => Some(v),
            None => best_version(core, platform, &pid, &mc, &loaders).await.ok().flatten(),
        };
        if let Some(v) = &ver {
            if depth < 6 {
                for d in v.dependencies.iter().filter(|d| d.kind == "required") {
                    queue.push_back((d.clone(), depth + 1));
                }
            }
        }
        let note = match &ver {
            None => Some(format!("没有找到适用于 Minecraft {mc} 的版本，请手动处理")),
            Some(v) if v.primary_file().and_then(|f| f.url.as_ref()).is_none() => {
                Some("作者不允许第三方下载，需要手动下载".into())
            }
            _ => None,
        };
        out.push(PlanItem {
            platform,
            project_id: pid,
            title: dep_title,
            icon_url: dep_icon,
            version: ver,
            root: false,
            installed: false,
            note,
        });
    }
    Ok(out)
}

// ───────────────────────── 安装到实例 ─────────────────────────

pub fn start_install(
    core: &Arc<Core>,
    instance: String,
    kind: ResourceKind,
    save: Option<String>,
    items: Vec<InstallItem>,
) -> anyhow::Result<String> {
    if items.is_empty() {
        bail!("没有要安装的文件");
    }
    if kind == ResourceKind::Modpack {
        bail!("整合包请使用“安装整合包”，它会创建新的实例");
    }
    let dirs = core.game_dirs();
    let inst_dir = dirs.instance_dir(&instance);
    if !inst_dir.is_dir() {
        bail!("实例 {instance} 不存在");
    }
    let target = target_dir(&inst_dir, kind, save.as_deref())?;
    let mut blocked = Vec::new();
    for it in &items {
        let f = it.version.primary_file().ok_or_else(|| anyhow!("{} 没有可下载的文件", it.title))?;
        safe_file_name(&f.filename)?;
        if f.url.is_none() {
            blocked.push(it.title.clone());
        }
    }
    if !blocked.is_empty() {
        bail!(
            "以下资源的作者不允许第三方启动器下载，请到项目页手动下载后放入对应文件夹：{}",
            blocked.join("、")
        );
    }
    let title = if items.len() == 1 {
        format!("安装{} {} 到 {instance}", kind.label(), items[0].title)
    } else {
        format!("安装 {} 等 {} 个{}到 {instance}", items[0].title, items.len(), kind.label())
    };
    let task = core.tasks.create(title, "community");
    let id = task.id.clone();
    let core2 = core.clone();
    spawn_task(task, move |task| async move {
        let label = format!("下载{}", kind.label());
        install_items(&core2, &task, &instance, kind, target, items, (1, 1), label).await
    });
    Ok(id)
}

/// 随加载器一起安装的前置模组（Fabric API / QFAPI）：在加载器安装任务中作为一个阶段下载到 mods。
/// 与普通社区资源安装走同一套代码（MCIM 源策略、哈希校验、glass-resources.json 记录）；
/// 另外会删除 mods 里同一模组的其他版本（包括手动放入、没有记录的文件），避免重复加载。
pub async fn install_addons(
    core: &Core,
    task: &Arc<TaskHandle>,
    instance: &str,
    items: Vec<InstallItem>,
    stage: (u32, u32),
) -> anyhow::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let inst_dir = core.game_dirs().instance_dir(instance);
    let target = target_dir(&inst_dir, ResourceKind::Mod, None)?;
    let mut keep: Vec<String> = Vec::new();
    let mut mod_ids: Vec<&'static str> = Vec::new();
    for it in &items {
        if it.version.platform != Platform::Modrinth {
            bail!("{} 只支持从 Modrinth 安装", it.title);
        }
        let f = it.version.primary_file().ok_or_else(|| anyhow!("{} 没有可下载的文件", it.title))?;
        if f.url.is_none() {
            bail!("{} 没有下载地址", it.title);
        }
        keep.push(safe_file_name(&f.filename)?.to_ascii_lowercase());
        mod_ids.extend_from_slice(addon_mod_ids(&it.version.project_id));
    }
    let titles = items.iter().map(|i| i.title.clone()).collect::<Vec<_>>().join("、");
    install_items(core, task, instance, ResourceKind::Mod, target.clone(), items, stage, format!("下载 {titles}")).await?;

    if !mod_ids.is_empty() {
        if let Ok(list) = local::list(core, instance, ResourceKind::Mod) {
            for r in list {
                let same = r.mod_id.as_deref().map(|id| mod_ids.iter().any(|m| *m == id)).unwrap_or(false);
                let name = r.file.to_ascii_lowercase();
                let base = name.strip_suffix(".disabled").unwrap_or(&name).to_string();
                if same && !r.is_dir && !keep.contains(&base) {
                    let _ = std::fs::remove_file(target.join(&r.file));
                }
            }
        }
    }
    Ok(())
}

/// 前置模组在 fabric.mod.json / quilt.mod.json 中的 id（用于清理旧版本）
fn addon_mod_ids(project_id: &str) -> &'static [&'static str] {
    match project_id {
        // Fabric API（新版 id 为 fabric-api，很早的版本为 fabric）
        "P7dR8mSH" => &["fabric-api", "fabric"],
        // Quilted Fabric API / QSL
        "qvIfYCYJ" => &["quilted_fabric_api", "qsl"],
        _ => &[],
    }
}

fn options(core: &Core) -> DownloadOptions {
    let s = core.settings();
    DownloadOptions { concurrency: s.concurrency, retries: s.retries, verify: Verify::Full }
}

/// sha1 缺失、只有 sha512 时（部分 Modrinth 文件）在下载后补充校验
pub async fn verify_sha512(path: &Path, expected: &str) -> anyhow::Result<()> {
    let p = path.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use sha2::{Digest, Sha512};
        use std::io::Read;
        let mut f = std::fs::File::open(&p)?;
        let mut h = Sha512::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Ok(hex::encode(h.finalize()))
    })
    .await??;
    if !actual.eq_ignore_ascii_case(expected) {
        let _ = std::fs::remove_file(path);
        bail!("{} SHA-512 校验失败", path.display());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn install_items(
    core: &Core,
    task: &Arc<TaskHandle>,
    instance: &str,
    kind: ResourceKind,
    target: PathBuf,
    items: Vec<InstallItem>,
    stage: (u32, u32),
    label: String,
) -> anyhow::Result<()> {
    let inst_dir = core.game_dirs().instance_dir(instance);
    if kind == ResourceKind::World {
        // 地图：先下载 zip 到缓存，再解压到 saves
        let total = items.len() as u32 * 2;
        let mut stage = 0;
        for it in &items {
            let f = it.version.primary_file().ok_or_else(|| anyhow!("没有文件"))?;
            let name = safe_file_name(&f.filename)?;
            let zip = core.paths.cache_dir().join("downloads").join(&name);
            stage += 1;
            task.set_stage(stage, total, format!("下载地图 {}", it.title), 1, f.size);
            let item = DownloadItem {
                urls: urls(core, f.url.as_deref().unwrap_or_default()),
                path: zip.clone(),
                sha1: f.sha1.clone(),
                size: (f.size > 0).then_some(f.size),
            };
            download_all(&core.http, vec![item], options(core), task).await?;
            stage += 1;
            let created = world::import_zip(task, &zip, &target, (stage, total), Some(it.title.as_str())).await?;
            let _ = std::fs::remove_file(&zip);
            if created.is_empty() {
                bail!("{} 中没有找到存档（level.dat）", it.title);
            }
        }
        return Ok(());
    }

    std::fs::create_dir_all(&target)?;
    let mut downloads = Vec::new();
    let mut records = Vec::new();
    for it in &items {
        let f = it.version.primary_file().ok_or_else(|| anyhow!("没有文件"))?;
        let name = safe_file_name(&f.filename)?;
        let path = target.join(&name);
        downloads.push(DownloadItem {
            urls: urls(core, f.url.as_deref().unwrap_or_default()),
            path: path.clone(),
            sha1: f.sha1.clone(),
            size: (f.size > 0).then_some(f.size),
        });
        records.push((it, f.clone(), path));
    }
    task.set_stage(stage.0, stage.1, label, downloads.len() as u64, total_size(&downloads));
    download_all(&core.http, downloads, options(core), task).await?;

    let mut index = ResourceIndex::load(&inst_dir);
    for (it, f, path) in records {
        if f.sha1.is_none() {
            if let Some(h) = &f.sha512 {
                verify_sha512(&path, h).await?;
            }
        }
        let rel = path
            .strip_prefix(&inst_dir)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| f.filename.clone());
        let parent_rel = rel.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default();
        // 更新：同一项目在同一目录的旧文件删除
        let mut keep = Vec::new();
        for e in index.entries.drain(..) {
            let same_project = e.kind == kind && e.platform == it.version.platform && e.project_id == it.version.project_id;
            let same_dir = e.path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default() == parent_rel;
            if same_project && same_dir {
                if !e.path.eq_ignore_ascii_case(&rel) {
                    let old = inst_dir.join(&e.path);
                    let _ = std::fs::remove_file(&old);
                    let _ = std::fs::remove_file(format!("{}.disabled", old.display()));
                }
                continue;
            }
            keep.push(e);
        }
        index.entries = keep;
        index.entries.push(IndexEntry {
            kind,
            path: rel,
            platform: it.version.platform,
            project_id: it.version.project_id.clone(),
            version_id: it.version.id.clone(),
            title: it.title.clone(),
            icon_url: it.icon_url.clone(),
            installed_at: now_ms(),
        });
    }
    index.save(&inst_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datapack_target() {
        let d = target_dir(Path::new("inst"), ResourceKind::Datapack, Some("World1")).unwrap();
        assert_eq!(d, Path::new("inst").join("saves").join("World1").join("datapacks"));
        assert!(target_dir(Path::new("inst"), ResourceKind::Datapack, None).is_err());
    }

    #[test]
    fn file_names() {
        assert!(safe_file_name("sodium-0.5.jar").is_ok());
        assert!(safe_file_name("../evil.jar").is_err());
        assert!(safe_file_name("a/b.jar").is_err());
    }
}
