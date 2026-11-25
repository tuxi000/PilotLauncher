//! 整合包：Modrinth .mrpack 与 CurseForge 整合包 zip（在线安装与本地导入共用同一流程）。
//!
//! .mrpack：modrinth.index.json → 按 dependencies 创建实例并安装加载器 → 下载 files
//!          （校验 sha1 / sha512，跳过 env.client = unsupported）→ 解压 overrides、client-overrides。
//! CurseForge：manifest.json → 按 minecraft.modLoaders 安装加载器 → /v1/mods/files 批量取下载地址
//!          → 不允许第三方下载（downloadUrl 为 null）的文件列出来提示手动下载 → 解压 overrides。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Emitter;

use super::world::{find_world_roots, safe_entry_path};
use super::{curseforge, safe_file_name, urls, verify_sha512, IndexEntry, Platform, ResourceIndex, ResourceKind, ResourceVersion};
use crate::download::{download_all, total_size, DownloadItem, DownloadOptions, Verify};
use crate::instance;
use crate::minecraft::install::install_vanilla_staged;
use crate::modloader::{self, LoaderKind, LoaderSpec, Stager};
use crate::state::Core;
use crate::tasks::{spawn_task, TaskHandle};
use crate::util::{now_ms, read_json, validate_instance_name, write_json_atomic};

pub const MISSING_FILE: &str = "glass-missing-files.json";
pub const MISSING_EVENT: &str = "modpack-missing";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MissingFile {
    pub name: String,
    pub file_name: String,
    /// 项目页 / 文件页，用户手动下载
    pub url: String,
    /// 下载后应放入的实例子目录
    pub target: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MissingEvent {
    pub instance: String,
    pub files: Vec<MissingFile>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveInfo {
    /// mrpack / curseforge / world / unknown
    pub kind: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub mc_version: Option<String>,
    pub loaders: Vec<String>,
    pub worlds: Vec<String>,
    pub file_count: usize,
    pub summary: Option<String>,
}

enum Pack {
    Modrinth(Value),
    Curse(Value),
}

fn read_text(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut e = zip.by_name(name).ok()?;
    let mut b = Vec::new();
    e.read_to_end(&mut b).ok()?;
    Some(b)
}

fn read_pack(path: &Path) -> anyhow::Result<Pack> {
    let f = std::fs::File::open(path).with_context(|| format!("无法打开 {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(f).context("不是有效的整合包（zip）")?;
    if let Some(b) = read_text(&mut zip, "modrinth.index.json") {
        return Ok(Pack::Modrinth(serde_json::from_slice(&b).context("modrinth.index.json 格式错误")?));
    }
    if let Some(b) = read_text(&mut zip, "manifest.json") {
        let v: Value = serde_json::from_slice(&b).context("manifest.json 格式错误")?;
        if v.get("minecraft").is_some() {
            return Ok(Pack::Curse(v));
        }
    }
    bail!("没有找到 modrinth.index.json 或 CurseForge 的 manifest.json，不是支持的整合包格式")
}

/// (Minecraft 版本, 加载器类型, 加载器版本) 列表
fn pack_loaders(pack: &Pack) -> anyhow::Result<(String, Vec<(LoaderKind, String)>)> {
    match pack {
        Pack::Modrinth(v) => {
            let deps = v.get("dependencies").cloned().unwrap_or_default();
            let mc = deps.get("minecraft").and_then(|x| x.as_str()).ok_or_else(|| anyhow!("整合包没有指定 Minecraft 版本"))?;
            let mut out = Vec::new();
            for (k, kind) in [
                ("forge", LoaderKind::Forge),
                ("neoforge", LoaderKind::NeoForge),
                ("fabric-loader", LoaderKind::Fabric),
                ("quilt-loader", LoaderKind::Quilt),
            ] {
                if let Some(ver) = deps.get(k).and_then(|x| x.as_str()) {
                    out.push((kind, ver.to_string()));
                }
            }
            Ok((mc.to_string(), out))
        }
        Pack::Curse(v) => {
            let mc = v
                .pointer("/minecraft/version")
                .and_then(|x| x.as_str())
                .ok_or_else(|| anyhow!("manifest.json 没有指定 Minecraft 版本"))?;
            let mut list: Vec<(bool, LoaderKind, String)> = Vec::new();
            for l in v.pointer("/minecraft/modLoaders").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
                let id = l.get("id").and_then(|x| x.as_str()).unwrap_or("");
                let primary = l.get("primary").and_then(|x| x.as_bool()).unwrap_or(false);
                let Some((k, ver)) = id.split_once('-') else { continue };
                let Some(kind) = LoaderKind::from_id(k) else { continue };
                list.push((primary, kind, ver.to_string()));
            }
            list.sort_by_key(|x| !x.0);
            Ok((mc.to_string(), list.into_iter().take(1).map(|x| (x.1, x.2)).collect()))
        }
    }
}

/// 识别本地文件：整合包 / 地图 / 未知（拖放导入时使用）
pub fn inspect(path: &Path) -> anyhow::Result<ArchiveInfo> {
    let mut info = ArchiveInfo { kind: "unknown".into(), ..Default::default() };
    if let Ok(pack) = read_pack(path) {
        let (mc, loaders) = pack_loaders(&pack)?;
        info.mc_version = Some(mc);
        info.loaders = loaders.iter().map(|(k, v)| format!("{} {v}", k.display_name())).collect();
        match &pack {
            Pack::Modrinth(v) => {
                info.kind = "mrpack".into();
                info.name = super::os(v, "name");
                info.version = super::os(v, "versionId");
                info.summary = super::os(v, "summary");
                info.file_count = v.get("files").and_then(|f| f.as_array()).map(|a| a.len()).unwrap_or(0);
            }
            Pack::Curse(v) => {
                info.kind = "curseforge".into();
                info.name = super::os(v, "name");
                info.version = super::os(v, "version");
                info.summary = super::os(v, "author").map(|a| format!("作者：{a}"));
                info.file_count = v.get("files").and_then(|f| f.as_array()).map(|a| a.len()).unwrap_or(0);
            }
        }
        return Ok(info);
    }
    let f = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(f).context("不是有效的 zip 文件")?;
    let mut names = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let e = zip.by_index(i)?;
        names.push(safe_entry_path(e.enclosed_name()).unwrap_or_default());
    }
    let roots = find_world_roots(&names);
    if !roots.is_empty() {
        info.kind = "world".into();
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "World".into());
        info.worlds = roots
            .iter()
            .map(|r| {
                if r.is_empty() {
                    stem.clone()
                } else {
                    r.trim_end_matches('/').rsplit('/').next().unwrap_or("World").to_string()
                }
            })
            .collect();
        info.name = Some(stem);
        info.file_count = names.len();
    }
    Ok(info)
}

/// 校验整合包中的相对路径
fn safe_rel(p: &str) -> Option<String> {
    let n = p.replace('\\', "/");
    if n.is_empty() || n.starts_with('/') || n.contains(':') || n.split('/').any(|c| c == ".." || c.is_empty()) {
        return None;
    }
    Some(n)
}

fn options(core: &Core) -> DownloadOptions {
    let s = core.settings();
    DownloadOptions { concurrency: s.concurrency, retries: s.retries, verify: Verify::Full }
}

fn check_new_name(core: &Core, name: &str) -> anyhow::Result<()> {
    validate_instance_name(name)?;
    if core.game_dirs().instance_dir(name).exists() {
        bail!("已存在同名实例或文件夹：{name}");
    }
    Ok(())
}

/// 导入本地整合包（.mrpack 或 CurseForge zip），返回任务 id
pub fn start_import(core: &Arc<Core>, path: String, name: String) -> anyhow::Result<String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    if !p.is_file() {
        bail!("文件不存在：{}", p.display());
    }
    let name = name.trim().to_string();
    check_new_name(core, &name)?;
    let task = core.tasks.create(format!("导入整合包 {name}"), "install");
    let id = task.id.clone();
    let core2 = core.clone();
    spawn_task(task, move |task| async move { import_pack(&core2, &task, &p, &name, 0).await });
    Ok(id)
}

/// 在线安装整合包：先下载整合包文件，再走导入流程
pub fn start_remote(core: &Arc<Core>, version: ResourceVersion, name: String, title: String) -> anyhow::Result<String> {
    let name = name.trim().to_string();
    check_new_name(core, &name)?;
    let file = version.primary_file().cloned().ok_or_else(|| anyhow!("该版本没有文件"))?;
    let url = file
        .url
        .clone()
        .ok_or_else(|| anyhow!("该整合包的作者不允许第三方启动器下载，请到项目页手动下载后使用“导入本地整合包”"))?;
    let file_name = safe_file_name(&file.filename)?;
    let task = core.tasks.create(format!("安装整合包 {title}（{name}）"), "install");
    let id = task.id.clone();
    let core2 = core.clone();
    spawn_task(task, move |task| async move {
        let path = core2.paths.cache_dir().join("modpacks").join(&file_name);
        task.set_stage(1, 2, "下载整合包", 1, file.size);
        let item = DownloadItem {
            urls: urls(&core2, &url),
            path: path.clone(),
            sha1: file.sha1.clone(),
            size: (file.size > 0).then_some(file.size),
        };
        download_all(&core2.http, vec![item], options(&core2), &task).await?;
        if file.sha1.is_none() {
            if let Some(h) = &file.sha512 {
                verify_sha512(&path, h).await?;
            }
        }
        let r = import_pack(&core2, &task, &path, &name, 1).await;
        let _ = std::fs::remove_file(&path);
        r
    });
    Ok(id)
}

async fn resolve_specs(core: &Core, mc: &str, loaders: &[(LoaderKind, String)]) -> anyhow::Result<Vec<LoaderSpec>> {
    let mut out = Vec::new();
    for (kind, ver) in loaders {
        let id = match kind {
            LoaderKind::Fabric | LoaderKind::Quilt => ver.clone(),
            _ => modloader::find_version(core, *kind, mc, ver).await?.id,
        };
        out.push(LoaderSpec { kind: *kind, version: id });
    }
    Ok(out)
}

async fn import_pack(core: &Arc<Core>, task: &Arc<TaskHandle>, path: &Path, name: &str, done: u32) -> anyhow::Result<()> {
    task.set_stage(done + 1, done + 2, "解析整合包", 0, 0);
    let p2 = path.to_path_buf();
    let pack = tokio::task::spawn_blocking(move || read_pack(&p2)).await??;
    let (mc, raw_loaders) = pack_loaders(&pack)?;
    let specs = resolve_specs(core, &mc, &raw_loaders).await?;
    modloader::validate(&specs)?;
    let loader_stages = if specs.is_empty() { 0 } else { modloader::stage_count(&specs) };
    // 文件阶段：mrpack 为「下载 + 解压」，CurseForge 多一个「获取文件信息」
    let file_stages = if matches!(pack, Pack::Curse(_)) { 3 } else { 2 };
    let total = done + 1 + 3 + loader_stages + file_stages;
    task.set_stage(done + 1, total, "解析整合包", 0, 0);

    // 实例
    instance::prepare(core, name, &mc)?;
    let dirs = core.game_dirs();
    let inst_dir = dirs.instance_dir(name);
    {
        let mut meta = instance::load_meta(&dirs, name)?;
        let (pname, pver) = match &pack {
            Pack::Modrinth(v) => (super::s(v, "name"), super::s(v, "versionId")),
            Pack::Curse(v) => (super::s(v, "name"), super::s(v, "version")),
        };
        meta.description = format!("整合包：{pname} {pver}").trim().to_string();
        // 自动图标：显示整合包所用加载器的图标
        meta.icon = instance::AUTO_ICON.into();
        instance::save_meta(&dirs, &meta)?;
    }
    install_vanilla_staged(core, task, &mc, name, done + 2, total).await?;
    let mut st = Stager::new(task.clone(), done + 4, total);
    if !specs.is_empty() {
        modloader::install_loaders(core, task, name, &specs, &mut st).await?;
    }

    // 文件
    let overrides: Vec<String> = match &pack {
        Pack::Modrinth(index) => {
            mr_files(core, task, &mut st, index, &inst_dir).await?;
            vec!["overrides/".into(), "client-overrides/".into()]
        }
        Pack::Curse(manifest) => {
            let missing = cf_files(core, task, &mut st, manifest, &inst_dir).await?;
            if !missing.is_empty() {
                write_json_atomic(&inst_dir.join(MISSING_FILE), &missing)?;
                let _ = core.app.emit(MISSING_EVENT, MissingEvent { instance: name.to_string(), files: missing });
            }
            let o = manifest.get("overrides").and_then(|x| x.as_str()).unwrap_or("overrides").trim_matches('/').to_string();
            vec![format!("{o}/")]
        }
    };

    // 覆盖文件
    let stage = st.next("解压覆盖文件（overrides）", 0, 0);
    let (zp, dest, t) = (path.to_path_buf(), inst_dir.clone(), task.clone());
    tokio::task::spawn_blocking(move || extract_overrides(&zp, &overrides, &dest, &t, stage)).await??;
    Ok(())
}

async fn mr_files(core: &Core, task: &Arc<TaskHandle>, st: &mut Stager, index: &Value, inst_dir: &Path) -> anyhow::Result<()> {
    let mut items = Vec::new();
    let mut sha512_only: Vec<(PathBuf, String)> = Vec::new();
    for f in index.get("files").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
        if f.pointer("/env/client").and_then(|x| x.as_str()) == Some("unsupported") {
            continue;
        }
        let Some(rel) = safe_rel(&super::s(&f, "path")) else {
            bail!("整合包中有非法路径：{}", super::s(&f, "path"));
        };
        let path = inst_dir.join(rel.replace('/', "\\"));
        let mut list: Vec<String> = Vec::new();
        for u in f.get("downloads").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
            if let Some(u) = u.as_str() {
                for c in urls(core, u) {
                    if !list.contains(&c) {
                        list.push(c);
                    }
                }
            }
        }
        if list.is_empty() {
            bail!("{rel} 没有下载地址");
        }
        let sha1 = f.pointer("/hashes/sha1").and_then(|x| x.as_str()).map(|x| x.to_string());
        if sha1.is_none() {
            if let Some(h) = f.pointer("/hashes/sha512").and_then(|x| x.as_str()) {
                sha512_only.push((path.clone(), h.to_string()));
            }
        }
        items.push(DownloadItem { urls: list, path, sha1, size: f.get("fileSize").and_then(|x| x.as_u64()) });
    }
    st.next("下载整合包文件", items.len() as u64, total_size(&items));
    download_all(&core.http, items, options(core), task).await?;
    for (p, h) in sha512_only {
        verify_sha512(&p, &h).await?;
    }
    Ok(())
}

async fn cf_files(
    core: &Core,
    task: &Arc<TaskHandle>,
    st: &mut Stager,
    manifest: &Value,
    inst_dir: &Path,
) -> anyhow::Result<Vec<MissingFile>> {
    let entries: Vec<(u64, u64, bool)> = manifest
        .get("files")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|f| {
            Some((
                f.get("projectID")?.as_u64()?,
                f.get("fileID")?.as_u64()?,
                f.get("required").and_then(|x| x.as_bool()).unwrap_or(true),
            ))
        })
        .filter(|e| e.2)
        .collect();
    st.next("获取 CurseForge 文件信息", 0, 0);
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let file_ids: Vec<u64> = entries.iter().map(|e| e.1).collect();
    let project_ids: Vec<u64> = entries.iter().map(|e| e.0).collect();
    let infos = curseforge::files_batch(core, &file_ids).await?;
    let mods = curseforge::mods_batch(core, &project_ids).await.unwrap_or_default();
    let by_id: HashMap<u64, Value> = infos
        .into_iter()
        .filter_map(|f| Some((f.get("id")?.as_u64()?, f)))
        .collect();

    let mut items = Vec::new();
    let mut missing = Vec::new();
    let mut index = ResourceIndex::load(inst_dir);
    for (pid, fid, _) in &entries {
        let (mod_name, page, class) = mods.get(pid).cloned().unwrap_or_else(|| (format!("项目 {pid}"), String::new(), 6));
        let sub = match class {
            12 => "resourcepacks",
            6552 => "shaderpacks",
            _ => "mods",
        };
        let page_url = if page.is_empty() {
            format!("https://www.curseforge.com/projects/{pid}")
        } else {
            format!("{page}/files/{fid}")
        };
        let Some(info) = by_id.get(fid) else {
            missing.push(MissingFile { name: mod_name, file_name: format!("文件 {fid}"), url: page_url, target: sub.into() });
            continue;
        };
        let ver = curseforge::map_file(info);
        let Some(file) = ver.primary_file().cloned() else { continue };
        let Ok(fname) = safe_file_name(&file.filename) else { continue };
        match &file.url {
            None => missing.push(MissingFile { name: mod_name, file_name: fname, url: page_url, target: sub.into() }),
            Some(u) => {
                let path = inst_dir.join(sub).join(&fname);
                items.push(DownloadItem {
                    urls: urls(core, u),
                    path,
                    sha1: file.sha1.clone(),
                    size: (file.size > 0).then_some(file.size),
                });
                let kind = match class {
                    12 => ResourceKind::Resourcepack,
                    6552 => ResourceKind::Shaderpack,
                    _ => ResourceKind::Mod,
                };
                index.entries.push(IndexEntry {
                    kind,
                    path: format!("{sub}/{fname}"),
                    platform: Platform::Curseforge,
                    project_id: pid.to_string(),
                    version_id: fid.to_string(),
                    title: mod_name,
                    icon_url: None,
                    installed_at: now_ms(),
                });
            }
        }
    }
    st.next("下载整合包文件", items.len() as u64, total_size(&items));
    download_all(&core.http, items, options(core), task).await?;
    index.save(inst_dir)?;
    Ok(missing)
}

/// 依次解压各 overrides 前缀（后面的覆盖前面的），防 zip slip
fn extract_overrides(
    zip_path: &Path,
    prefixes: &[String],
    dest: &Path,
    task: &TaskHandle,
    stage: (u32, u32),
) -> anyhow::Result<()> {
    let f = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    let mut names = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let e = zip.by_index(i)?;
        names.push((safe_entry_path(e.enclosed_name()).unwrap_or_default(), e.is_dir(), e.size()));
    }
    let count = names
        .iter()
        .filter(|(n, d, _)| !d && prefixes.iter().any(|p| n.starts_with(p.as_str())))
        .count();
    task.set_stage(stage.0, stage.1, "解压覆盖文件（overrides）", count as u64, 0);
    for prefix in prefixes {
        for i in 0..names.len() {
            if task.is_cancelled() {
                bail!("任务已取消");
            }
            let (n, is_dir, _) = &names[i];
            let Some(rel) = n.strip_prefix(prefix.as_str()) else { continue };
            if rel.is_empty() {
                continue;
            }
            let out = dest.join(rel.replace('/', "\\"));
            if !out.starts_with(dest) {
                continue;
            }
            if *is_dir {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut e = zip.by_index(i)?;
            let mut w = std::fs::File::create(&out).with_context(|| format!("写入 {} 失败", out.display()))?;
            std::io::copy(&mut e, &mut w)?;
            task.add_progress(1, 0);
        }
    }
    Ok(())
}

pub fn read_missing(core: &Core, instance: &str) -> Vec<MissingFile> {
    read_json(&core.game_dirs().instance_dir(instance).join(MISSING_FILE)).unwrap_or_default()
}

pub fn dismiss_missing(core: &Core, instance: &str) -> anyhow::Result<()> {
    let p = core.game_dirs().instance_dir(instance).join(MISSING_FILE);
    if p.is_file() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}
