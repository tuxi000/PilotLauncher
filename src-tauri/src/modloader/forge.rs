//! Forge / NeoForge：版本列表 + 安装器处理（新版 processors 与旧版 versionInfo 共用一套代码）。
//!
//! 新版安装器（Forge 1.12.2-14.23.5.2851+ / 1.13+，NeoForge 全部）：
//!   1. 下载 installer.jar，读取 install_profile.json 与 version.json
//!   2. 下载 install_profile.libraries + version.libraries（maven 镜像由 download::source 替换），
//!      url 为空的库从安装器内的 maven/ 目录解出（其余由 processors 生成）
//!   3. 解析 data 变量：[maven 坐标] → 库路径，'字面量'，/data/xxx → 从安装器解出到临时目录；
//!      内置 {SIDE} {MINECRAFT_JAR} {MINECRAFT_VERSION} {ROOT} {INSTALLER} {LIBRARY_DIR}
//!   4. 依次运行 sides 包含 client（或未指定 sides）的 processor：主类取自 jar 的 MANIFEST Main-Class，
//!      classpath = processor jar + 它的 classpath；已存在且 sha1 正确的 outputs 跳过；运行后校验 outputs 的 sha1。
//!      DOWNLOAD_MOJMAPS 任务由启动器自己按下载源下载 client_mappings（避免 Java 直连 Mojang）。
//!   5. version.json 作为子版本合并进实例 JSON
//! 旧版安装器（1.12.2 及以前）：install_profile.json 中的 versionInfo 即版本 JSON，install.filePath 为 universal jar。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use serde_json::Value;

use super::{cmp_version, console_java, download_options, instance_java, merge_json, meta, run_java, LoaderKind, LoaderVersion, Stager};
use crate::download::source::BMCLAPI;
use crate::download::{download_all, total_size, DownloadItem, Verify};
use crate::minecraft::library::MavenCoord;
use crate::minecraft::version::Library;
use crate::settings::DownloadSource;
use crate::state::Core;
use crate::tasks::TaskHandle;
use crate::util::file_sha1;

pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
pub const NEO_MAVEN: &str = "https://maven.neoforged.net/releases";
const FORGE_METADATA: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEO_API: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged";

// ───────────────────────── Forge 版本列表 ─────────────────────────
//
// 数据来源（均带 1 小时内存 + 磁盘缓存，见 meta.rs）：
//  - 官方：maven-metadata.xml（包含所有 Minecraft 版本，只下载一次、所有版本共用）+ promotions_slim.json（推荐版）
//  - BMCLAPI：/forge/minecraft/<mc>（只含该版本，体积小）+ /forge/promos
// “官方优先”时两边同时请求、谁先成功用谁；版本列表与推荐版信息也并行获取。

fn forge_from_bmcl(mc: &str, items: &[Value]) -> Vec<LoaderVersion> {
    let mut out: Vec<(i64, LoaderVersion)> = Vec::new();
    for it in items {
        let Some(ver) = it.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(files) = it.get("files").and_then(|f| f.as_array()) {
            if !files.is_empty()
                && !files.iter().any(|f| f.get("category").and_then(|c| c.as_str()) == Some("installer"))
            {
                // 没有安装器的远古版本不支持
                continue;
            }
        }
        let branch = it.get("branch").and_then(|b| b.as_str()).filter(|b| !b.is_empty());
        let id = match branch {
            Some(b) => format!("{mc}-{ver}-{b}"),
            None => format!("{mc}-{ver}"),
        };
        let build = it.get("build").and_then(|b| b.as_i64()).unwrap_or(0);
        out.push((
            build,
            LoaderVersion {
                kind: LoaderKind::Forge,
                id,
                version: ver.to_string(),
                mc_version: mc.to_string(),
                stable: true,
                recommended: false,
                latest: false,
                note: it.get("modified").and_then(|m| m.as_str()).map(|m| m.chars().take(10).collect()),
            },
        ));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| cmp_version(&b.1.version, &a.1.version)));
    out.into_iter().map(|x| x.1).collect()
}

fn forge_from_metadata(mc: &str, xml: &str) -> Vec<LoaderVersion> {
    let prefix = format!("{mc}-");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<version>") {
        let after = &rest[start + 9..];
        let Some(end) = after.find("</version>") else { break };
        let id = after[..end].trim();
        rest = &after[end + 10..];
        if !id.starts_with(&prefix) {
            continue;
        }
        let ver = super::display_version(LoaderKind::Forge, mc, id);
        out.push(LoaderVersion {
            kind: LoaderKind::Forge,
            id: id.to_string(),
            version: ver,
            mc_version: mc.to_string(),
            stable: true,
            recommended: false,
            latest: false,
            note: None,
        });
    }
    out.sort_by(|a, b| cmp_version(&b.version, &a.version));
    out
}

async fn forge_official(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let xml = meta::cached(core, "forge-maven-metadata", force, || {
        meta::fetch_pair(core, FORGE_METADATA, None, meta::is_maven_metadata)
    })
    .await?;
    Ok(forge_from_metadata(mc, &String::from_utf8_lossy(xml.as_slice())))
}

async fn forge_bmcl(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let url = format!("{BMCLAPI}/forge/minecraft/{mc}");
    let key = format!("forge-bmcl-{mc}");
    let data = meta::cached(core, &key, force, || meta::fetch_only(core, &url, meta::is_json_array)).await?;
    let items: Vec<Value> = serde_json::from_slice(data.as_slice())?;
    Ok(forge_from_bmcl(mc, &items))
}

/// BMCLAPI 返回空列表时当作失败，让“官方优先”的竞速继续等官方结果
async fn forge_bmcl_nonempty(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let list = forge_bmcl(core, mc, force).await?;
    if list.is_empty() {
        bail!("BMCLAPI 没有 Forge {mc} 的版本");
    }
    Ok(list)
}

type Promo = (Option<String>, Option<String>);

async fn promos_official(core: &Core, mc: &str, force: bool) -> anyhow::Result<Promo> {
    let data = meta::cached(core, "forge-promotions-slim", force, || {
        meta::fetch_pair(core, FORGE_PROMOS, None, meta::is_json)
    })
    .await?;
    let v: Value = serde_json::from_slice(data.as_slice())?;
    let p = v.get("promos").cloned().unwrap_or_default();
    let get = |k: &str| p.get(format!("{mc}-{k}")).and_then(|x| x.as_str()).map(|s| s.to_string());
    Ok((get("recommended"), get("latest")))
}

async fn promos_bmcl(core: &Core, mc: &str, force: bool) -> anyhow::Result<Promo> {
    let url = format!("{BMCLAPI}/forge/promos");
    let data = meta::cached(core, "forge-promos-bmcl", force, || meta::fetch_only(core, &url, meta::is_json_array)).await?;
    let v: Value = serde_json::from_slice(data.as_slice())?;
    let mut rec = None;
    let mut lat = None;
    let rec_name = format!("{mc}-recommended");
    let lat_name = format!("{mc}-latest");
    for it in v.as_array().cloned().unwrap_or_default() {
        let name = it.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let ver = it.pointer("/build/version").and_then(|x| x.as_str()).map(|s| s.to_string());
        if name == rec_name {
            rec = ver;
        } else if name == lat_name {
            lat = ver;
        }
    }
    Ok((rec, lat))
}

/// (推荐版, 最新版) 的展示版本号；失败时返回 (None, None)（不影响列表）
async fn forge_promos(core: &Core, mc: &str, source: DownloadSource, force: bool) -> Promo {
    let r = match source {
        // 只用官方源时不访问 BMCLAPI
        DownloadSource::Official => promos_official(core, mc, force).await,
        DownloadSource::Bmclapi => match promos_bmcl(core, mc, force).await {
            Ok(p) => Ok(p),
            Err(_) => promos_official(core, mc, force).await,
        },
        DownloadSource::OfficialFirst => meta::first_ok(promos_official(core, mc, force), promos_bmcl(core, mc, force)).await,
    };
    r.unwrap_or((None, None))
}

pub async fn list_forge(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let source = core.settings().download_source;
    let list_fut = async {
        match source {
            DownloadSource::Official => forge_official(core, mc, force).await,
            DownloadSource::Bmclapi => match forge_bmcl(core, mc, force).await {
                Ok(v) if !v.is_empty() => Ok(v),
                _ => forge_official(core, mc, force).await,
            },
            DownloadSource::OfficialFirst => {
                meta::first_ok(forge_official(core, mc, force), forge_bmcl_nonempty(core, mc, force)).await
            }
        }
    };
    let (list, (rec, lat)) = tokio::join!(list_fut, forge_promos(core, mc, source, force));
    let mut list = list.context("获取 Forge 版本列表失败")?;
    for v in list.iter_mut() {
        v.recommended = rec.as_deref() == Some(v.version.as_str());
        v.latest = lat.as_deref() == Some(v.version.as_str());
    }
    Ok(list)
}

// ───────────────────────── NeoForge 版本列表 ─────────────────────────
//
//  - 官方：maven.neoforged.net/api/maven/versions/releases/net/neoforged/<neoforge|forge>
//    （JSON：{"isSnapshot":false,"versions":[...]}，包含所有版本，按前缀筛选；缓存后所有游戏版本共用）
//  - BMCLAPI：/neoforge/list/<mc>

/// NeoForge 版本前缀：1.20.2 → "20.2."，1.21 → "21.0."；26.1 → "26.1.0."，26.1.1 → "26.1.1."
pub fn neoforge_prefix(mc: &str) -> Option<String> {
    if mc == "1.20.1" {
        return Some("1.20.1-".into());
    }
    let all_num = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if let Some(rest) = mc.strip_prefix("1.") {
        let p: Vec<&str> = rest.split('.').collect();
        if p.is_empty() || p.len() > 2 || !p.iter().all(|x| all_num(x)) {
            return None;
        }
        return Some(format!("{}.{}.", p[0], p.get(1).copied().unwrap_or("0")));
    }
    let p: Vec<&str> = mc.split('.').collect();
    if p.len() < 2 || p.len() > 3 || !p.iter().all(|x| all_num(x)) {
        return None;
    }
    Some(format!("{}.{}.{}.", p[0], p[1], p.get(2).copied().unwrap_or("0")))
}

fn neo_version(mc: &str, id: &str) -> LoaderVersion {
    let lower = id.to_ascii_lowercase();
    LoaderVersion {
        kind: LoaderKind::NeoForge,
        id: id.to_string(),
        version: super::display_version(LoaderKind::NeoForge, mc, id),
        mc_version: mc.to_string(),
        stable: !(lower.contains("beta") || lower.contains("alpha") || lower.contains("snapshot")),
        recommended: false,
        latest: false,
        note: None,
    }
}

async fn list_neo_bmcl(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<String>> {
    let url = format!("{BMCLAPI}/neoforge/list/{mc}");
    let key = format!("neoforge-bmcl-{mc}");
    let data = meta::cached(core, &key, force, || meta::fetch_only(core, &url, meta::is_json_array)).await?;
    let v: Value = serde_json::from_slice(data.as_slice())?;
    let mut ids = Vec::new();
    for it in v.as_array().cloned().unwrap_or_default() {
        // installerPath：/maven/net/neoforged/neoforge/21.1.1/neoforge-21.1.1-installer.jar
        let from_path = it
            .get("installerPath")
            .and_then(|p| p.as_str())
            .and_then(|p| p.rsplit('/').nth(1).map(|s| s.to_string()));
        let id = from_path.or_else(|| it.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()));
        if let Some(id) = id {
            ids.push(id);
        }
    }
    Ok(ids)
}

async fn list_neo_bmcl_nonempty(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<String>> {
    let ids = list_neo_bmcl(core, mc, force).await?;
    if ids.is_empty() {
        bail!("BMCLAPI 没有 NeoForge {mc} 的版本");
    }
    Ok(ids)
}

async fn list_neo_official(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<String>> {
    let prefix = neoforge_prefix(mc).ok_or_else(|| anyhow!("NeoForge 暂不支持 Minecraft {mc}"))?;
    let artifact = if mc == "1.20.1" { "forge" } else { "neoforge" };
    let url = format!("{NEO_API}/{artifact}");
    let key = format!("neoforge-api-{artifact}");
    let data = meta::cached(core, &key, force, || meta::fetch_pair(core, &url, None, meta::is_json)).await?;
    let v: Value = serde_json::from_slice(data.as_slice())?;
    Ok(v.get("versions")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .filter(|s| s.starts_with(&prefix))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default())
}

pub async fn list_neoforge(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    if neoforge_prefix(mc).is_none() {
        bail!("NeoForge 暂不支持 Minecraft {mc}（NeoForge 从 1.20.1 开始提供）");
    }
    let mut ids = match core.settings().download_source {
        DownloadSource::Official => list_neo_official(core, mc, force).await?,
        DownloadSource::Bmclapi => match list_neo_bmcl(core, mc, force).await {
            Ok(v) if !v.is_empty() => v,
            _ => list_neo_official(core, mc, force).await?,
        },
        DownloadSource::OfficialFirst => {
            meta::first_ok(list_neo_official(core, mc, force), list_neo_bmcl_nonempty(core, mc, force))
                .await
                .context("获取 NeoForge 版本列表失败")?
        }
    };
    ids.sort_by(|a, b| cmp_version(b, a));
    ids.dedup();
    Ok(ids.iter().map(|id| neo_version(mc, id)).collect())
}

// ───────────────────────── 安装 ─────────────────────────

fn installer_coord(kind: LoaderKind, id: &str) -> (String, &'static str) {
    match kind {
        LoaderKind::NeoForge if id.starts_with("1.20.1-") => (format!("net.neoforged:forge:{id}:installer"), NEO_MAVEN),
        LoaderKind::NeoForge => (format!("net.neoforged:neoforge:{id}:installer"), NEO_MAVEN),
        _ => (format!("net.minecraftforge:forge:{id}:installer"), FORGE_MAVEN),
    }
}

/// 读取 zip 中的文本文件
fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut e = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    e.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// 解出 zip 中的单个文件
fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, dest: &Path) -> anyhow::Result<bool> {
    let mut e = match zip.by_name(name) {
        Ok(e) => e,
        Err(_) => return Ok(false),
    };
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = dest.with_extension("glass-tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        std::io::copy(&mut e, &mut f)?;
    }
    if dest.exists() {
        let _ = std::fs::remove_file(dest);
    }
    std::fs::rename(&tmp, dest)?;
    Ok(true)
}

struct InstallerData {
    profile: Value,
    version: Option<Value>,
}

fn read_installer(path: &Path) -> anyhow::Result<InstallerData> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).context("安装器文件已损坏")?;
    let profile_bytes = read_entry(&mut zip, "install_profile.json").ok_or_else(|| anyhow!("安装器中没有 install_profile.json"))?;
    let profile: Value = serde_json::from_slice(&profile_bytes).context("install_profile.json 格式错误")?;
    let version = if profile.get("versionInfo").is_some() {
        None
    } else {
        let name = profile
            .get("json")
            .and_then(|j| j.as_str())
            .unwrap_or("/version.json")
            .trim_start_matches('/')
            .to_string();
        let bytes = read_entry(&mut zip, &name).ok_or_else(|| anyhow!("安装器中没有 {name}"))?;
        Some(serde_json::from_slice(&bytes).with_context(|| format!("{name} 格式错误"))?)
    };
    Ok(InstallerData { profile, version })
}

#[allow(clippy::too_many_arguments)]
pub async fn install(
    core: &Arc<Core>,
    task: &Arc<TaskHandle>,
    st: &mut Stager,
    kind: LoaderKind,
    name: &str,
    mc: &str,
    id: &str,
    json: &mut Value,
) -> anyhow::Result<()> {
    let dirs = core.game_dirs();
    let (coord_s, repo) = installer_coord(kind, id);
    let coord = MavenCoord::parse(&coord_s).ok_or_else(|| anyhow!("无效的版本：{id}"))?;
    let installer = dirs.libraries().join(coord.path());
    let url = format!("{repo}/{}", coord.path());

    st.next(format!("下载 {} {} 安装器", kind.display_name(), super::display_version(kind, mc, id)), 1, 0);
    let item = DownloadItem { urls: core.sources(&url), path: installer.clone(), sha1: None, size: None };
    download_all(&core.http, vec![item], download_options(core, Verify::Quick), task).await?;

    let p2 = installer.clone();
    let data = match tokio::task::spawn_blocking(move || read_installer(&p2)).await? {
        Ok(d) => d,
        Err(e) => {
            // 损坏的安装器删除，下次重新下载
            let _ = std::fs::remove_file(&installer);
            return Err(e.context("读取安装器失败，已删除缓存，请重试"));
        }
    };
    match data.version {
        None => {
            st.skip(3);
            run_legacy(core, &installer, &data.profile, json).await
        }
        Some(version) => run_modern(core, task, st, kind, name, mc, &installer, &data.profile, &version, json).await,
    }
}

/// 旧版安装器：versionInfo + universal jar
async fn run_legacy(core: &Core, installer: &Path, profile: &Value, json: &mut Value) -> anyhow::Result<()> {
    let dirs = core.game_dirs();
    let install = profile.get("install").ok_or_else(|| anyhow!("install_profile.json 缺少 install 字段"))?;
    let path = install.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow!("缺少 install.path"))?;
    let file = install.get("filePath").and_then(|p| p.as_str()).ok_or_else(|| anyhow!("缺少 install.filePath"))?;
    let coord = MavenCoord::parse(path).ok_or_else(|| anyhow!("无效的坐标 {path}"))?;
    let dest = dirs.libraries().join(coord.path());
    let (inst, file_name, dest2) = (installer.to_path_buf(), file.to_string(), dest.clone());
    let found = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let f = std::fs::File::open(&inst)?;
        let mut zip = zip::ZipArchive::new(f)?;
        extract_entry(&mut zip, &file_name, &dest2)
    })
    .await??;
    if !found {
        bail!("安装器中没有 {file}");
    }

    let mut vi = profile.get("versionInfo").cloned().ok_or_else(|| anyhow!("缺少 versionInfo"))?;
    if let Some(libs) = vi.get_mut("libraries").and_then(|l| l.as_array_mut()) {
        // clientreq=false 的库只给服务端用
        libs.retain(|l| l.get("clientreq").and_then(|c| c.as_bool()) != Some(false));
    }
    *json = merge_json(json, &vi);
    Ok(())
}

/// Forge 的 replaceTokens：{KEY} 取变量，'xxx' 为字面量，反斜杠转义
fn replace_tokens(value: &str, data: &HashMap<String, String>) -> anyhow::Result<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    let mut x = 0;
    while x < chars.len() {
        let c = chars[x];
        if c == '\\' {
            x += 1;
            let n = *chars.get(x).ok_or_else(|| anyhow!("参数以反斜杠结尾：{value}"))?;
            out.push(n);
        } else if c == '{' || c == '\'' {
            let mut key = String::new();
            let mut y = x + 1;
            loop {
                let d = *chars.get(y).ok_or_else(|| anyhow!("参数中的括号未闭合：{value}"))?;
                if d == '\\' {
                    y += 1;
                    key.push(*chars.get(y).ok_or_else(|| anyhow!("参数以反斜杠结尾：{value}"))?);
                } else if (c == '{' && d == '}') || (c == '\'' && d == '\'') {
                    break;
                } else {
                    key.push(d);
                }
                y += 1;
            }
            x = y;
            if c == '\'' {
                out.push_str(&key);
            } else {
                let v = data.get(&key).ok_or_else(|| anyhow!("安装器变量 {{{key}}} 未定义"))?;
                out.push_str(v);
            }
        } else {
            out.push(c);
        }
        x += 1;
    }
    Ok(out)
}

/// processor 参数：[坐标] → 库路径，否则替换变量
fn resolve_arg(arg: &str, data: &HashMap<String, String>, lib_dir: &Path) -> anyhow::Result<String> {
    if arg.len() > 2 && arg.starts_with('[') && arg.ends_with(']') {
        let c = MavenCoord::parse(&arg[1..arg.len() - 1]).ok_or_else(|| anyhow!("无效的坐标 {arg}"))?;
        return Ok(lib_dir.join(c.path()).display().to_string());
    }
    replace_tokens(arg, data)
}

fn manifest_main_class(jar: &Path) -> anyhow::Result<String> {
    let f = std::fs::File::open(jar).with_context(|| format!("找不到 processor：{}", jar.display()))?;
    let mut zip = zip::ZipArchive::new(f).with_context(|| format!("processor 已损坏：{}", jar.display()))?;
    let bytes = read_entry(&mut zip, "META-INF/MANIFEST.MF").ok_or_else(|| anyhow!("{} 没有 MANIFEST", jar.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    // MANIFEST 的续行以空格开头
    let mut logical: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(last) = logical.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        logical.push(line.to_string());
    }
    logical
        .iter()
        .find_map(|l| l.strip_prefix("Main-Class:").map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("{} 的 MANIFEST 中没有 Main-Class", jar.display()))
}

#[allow(clippy::too_many_arguments)]
async fn run_modern(
    core: &Arc<Core>,
    task: &Arc<TaskHandle>,
    st: &mut Stager,
    kind: LoaderKind,
    name: &str,
    mc: &str,
    installer: &Path,
    profile: &Value,
    version: &Value,
    json: &mut Value,
) -> anyhow::Result<()> {
    let dirs = core.game_dirs();
    let lib_dir = dirs.libraries();

    // 1. 安装器自带的 maven/ 目录（url 为空的库）
    {
        let (inst, ld) = (installer.to_path_buf(), lib_dir.clone());
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let f = std::fs::File::open(&inst)?;
            let mut zip = zip::ZipArchive::new(f)?;
            for i in 0..zip.len() {
                let mut e = zip.by_index(i)?;
                if e.is_dir() {
                    continue;
                }
                let n = e.name().replace('\\', "/");
                let Some(rel) = n.strip_prefix("maven/") else { continue };
                if rel.is_empty() || rel.split('/').any(|c| c == ".." || c.is_empty()) {
                    continue;
                }
                let dest = ld.join(rel);
                if dest.is_file() && dest.metadata().map(|m| m.len()).unwrap_or(0) == e.size() {
                    continue;
                }
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut out = std::fs::File::create(&dest)?;
                std::io::copy(&mut e, &mut out)?;
            }
            Ok(())
        })
        .await??;
    }

    // 2. 依赖库
    let mut items = Vec::new();
    for arr in [profile.get("libraries"), version.get("libraries")] {
        for l in arr.and_then(|a| a.as_array()).cloned().unwrap_or_default() {
            let Ok(lib) = serde_json::from_value::<Library>(l) else { continue };
            let Some(coord) = MavenCoord::parse(&lib.name) else { continue };
            let art = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());
            let rel = art.and_then(|a| a.path.clone()).unwrap_or_else(|| coord.path());
            let path = lib_dir.join(&rel);
            let url = match art {
                Some(a) => a.url.clone().filter(|u| !u.trim().is_empty()),
                None => lib.url.as_ref().map(|base| {
                    if base.ends_with('/') {
                        format!("{base}{}", coord.path())
                    } else {
                        format!("{base}/{}", coord.path())
                    }
                }),
            };
            if let Some(u) = url {
                items.push(DownloadItem {
                    urls: core.sources(&u),
                    path,
                    sha1: art.and_then(|a| a.sha1.clone()),
                    size: art.and_then(|a| a.size),
                });
            }
        }
    }
    st.next(format!("下载 {} 依赖库", kind.display_name()), items.len() as u64, total_size(&items));
    download_all(&core.http, items, download_options(core, Verify::Full), task).await?;

    // 3. Java（与实例相同）
    let stage = st.next("准备 Java 运行时", 0, 0);
    let java_info = instance_java(core, task, name, stage).await?;
    let java = console_java(&java_info);

    // 4. data 变量
    let tmp = core.paths.cache_dir().join("installer-tmp").join(uuid::Uuid::new_v4().simple().to_string());
    std::fs::create_dir_all(&tmp)?;
    let mut data: HashMap<String, String> = HashMap::new();
    data.insert("SIDE".into(), "client".into());
    data.insert("MINECRAFT_JAR".into(), dirs.version_jar(name).display().to_string());
    data.insert("MINECRAFT_VERSION".into(), mc.to_string());
    data.insert("ROOT".into(), dirs.root.display().to_string());
    data.insert("INSTALLER".into(), installer.display().to_string());
    data.insert("LIBRARY_DIR".into(), lib_dir.display().to_string());
    let mut to_extract: Vec<(String, PathBuf)> = Vec::new();
    if let Some(obj) = profile.get("data").and_then(|d| d.as_object()) {
        for (k, v) in obj {
            let Some(val) = v.get("client").and_then(|c| c.as_str()) else { continue };
            let resolved = if val.len() > 2 && val.starts_with('[') && val.ends_with(']') {
                let c = MavenCoord::parse(&val[1..val.len() - 1]).ok_or_else(|| anyhow!("无效的坐标 {val}"))?;
                lib_dir.join(c.path()).display().to_string()
            } else if val.len() >= 2 && val.starts_with('\'') && val.ends_with('\'') {
                val[1..val.len() - 1].to_string()
            } else if let Some(inner) = val.strip_prefix('/') {
                if inner.split('/').any(|c| c == "..") {
                    bail!("非法的安装器路径 {val}");
                }
                let dest = tmp.join(inner);
                to_extract.push((inner.to_string(), dest.clone()));
                dest.display().to_string()
            } else {
                val.to_string()
            };
            data.insert(k.clone(), resolved);
        }
    }
    if !to_extract.is_empty() {
        let inst = installer.to_path_buf();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let f = std::fs::File::open(&inst)?;
            let mut zip = zip::ZipArchive::new(f)?;
            for (name, dest) in to_extract {
                if !extract_entry(&mut zip, &name, &dest)? {
                    bail!("安装器中缺少 /{name}");
                }
            }
            Ok(())
        })
        .await??;
    }

    // 5. processors
    let processors: Vec<Value> = profile
        .get("processors")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| match p.get("sides").and_then(|s| s.as_array()) {
            Some(sides) => sides.iter().any(|s| s.as_str() == Some("client")),
            None => true,
        })
        .collect();
    st.next(format!("运行安装器处理器（{} 个）", processors.len()), processors.len() as u64, 0);
    let result = run_processors(core, task, &java, &processors, &data, &lib_dir, &tmp, json).await;
    let _ = std::fs::remove_dir_all(&tmp);
    result?;

    *json = merge_json(json, version);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_processors(
    core: &Core,
    task: &Arc<TaskHandle>,
    java: &Path,
    processors: &[Value],
    data: &HashMap<String, String>,
    lib_dir: &Path,
    tmp: &Path,
    vanilla: &Value,
) -> anyhow::Result<()> {
    for (i, p) in processors.iter().enumerate() {
        if task.is_cancelled() {
            bail!("任务已取消");
        }
        let jar_name = p.get("jar").and_then(|j| j.as_str()).ok_or_else(|| anyhow!("processor 缺少 jar"))?;
        let jar = lib_dir.join(MavenCoord::parse(jar_name).ok_or_else(|| anyhow!("无效的坐标 {jar_name}"))?.path());
        let args: Vec<String> = p
            .get("args")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| resolve_arg(s, data, lib_dir)).collect::<anyhow::Result<Vec<_>>>())
            .transpose()?
            .unwrap_or_default();

        // outputs：{文件: sha1}
        let mut outputs: Vec<(PathBuf, String)> = Vec::new();
        if let Some(o) = p.get("outputs").and_then(|o| o.as_object()) {
            for (k, v) in o {
                let path = resolve_arg(k, data, lib_dir)?;
                let sha = resolve_arg(v.as_str().unwrap_or(""), data, lib_dir)?;
                outputs.push((PathBuf::from(path), sha));
            }
        }
        if !outputs.is_empty() {
            let mut all_ok = true;
            for (path, sha) in &outputs {
                let ok = path.is_file() && file_sha1(path).await.map(|s| s.eq_ignore_ascii_case(sha)).unwrap_or(false);
                if !ok {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                task.add_progress(1, 0);
                continue;
            }
        }

        // DOWNLOAD_MOJMAPS：由启动器按下载源下载官方映射表
        let task_pos = args.iter().position(|a| a == "--task");
        if task_pos.and_then(|i| args.get(i + 1)).map(|s| s == "DOWNLOAD_MOJMAPS").unwrap_or(false) {
            let out = args
                .iter()
                .position(|a| a == "--output")
                .and_then(|i| args.get(i + 1))
                .ok_or_else(|| anyhow!("DOWNLOAD_MOJMAPS 缺少 --output"))?;
            let m = vanilla
                .pointer("/downloads/client_mappings")
                .ok_or_else(|| anyhow!("原版 JSON 中没有 client_mappings"))?;
            let url = m.get("url").and_then(|u| u.as_str()).ok_or_else(|| anyhow!("client_mappings 缺少 url"))?;
            let item = DownloadItem {
                urls: core.sources(url),
                path: PathBuf::from(out),
                sha1: m.get("sha1").and_then(|s| s.as_str()).map(|s| s.to_string()),
                size: m.get("size").and_then(|s| s.as_u64()),
            };
            download_all(&core.http, vec![item], download_options(core, Verify::Full), task).await?;
            task.add_progress(1, 0);
            continue;
        }

        let main = {
            let j = jar.clone();
            tokio::task::spawn_blocking(move || manifest_main_class(&j)).await??
        };
        let mut cp = vec![jar.display().to_string()];
        for c in p.get("classpath").and_then(|c| c.as_array()).cloned().unwrap_or_default() {
            if let Some(s) = c.as_str() {
                let coord = MavenCoord::parse(s).ok_or_else(|| anyhow!("无效的坐标 {s}"))?;
                let path = lib_dir.join(coord.path());
                if !path.is_file() {
                    bail!("processor 依赖库缺失：{}", path.display());
                }
                cp.push(path.display().to_string());
            }
        }
        let mut full = vec!["-cp".to_string(), cp.join(";"), main.clone()];
        full.extend(args);
        run_java(java, &full, tmp, task)
            .await
            .with_context(|| format!("第 {} 个处理器 {main} 运行失败", i + 1))?;

        for (path, sha) in &outputs {
            if !path.is_file() {
                bail!("处理器 {main} 没有生成 {}", path.display());
            }
            let actual = file_sha1(path).await?;
            if !actual.eq_ignore_ascii_case(sha) {
                let _ = std::fs::remove_file(path);
                bail!("处理器 {main} 的输出 {} SHA-1 校验失败（期望 {sha}，实际 {actual}）", path.display());
            }
        }
        task.add_progress(1, 0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neo_prefix() {
        assert_eq!(neoforge_prefix("1.20.4").as_deref(), Some("20.4."));
        assert_eq!(neoforge_prefix("1.21").as_deref(), Some("21.0."));
        assert_eq!(neoforge_prefix("26.1").as_deref(), Some("26.1.0."));
        assert_eq!(neoforge_prefix("26.1.2").as_deref(), Some("26.1.2."));
        assert_eq!(neoforge_prefix("1.20.1").as_deref(), Some("1.20.1-"));
        assert!(neoforge_prefix("24w14a").is_none());
    }

    #[test]
    fn tokens() {
        let mut d = HashMap::new();
        d.insert("SIDE".to_string(), "client".to_string());
        assert_eq!(replace_tokens("--side={SIDE}", &d).unwrap(), "--side=client");
        assert_eq!(replace_tokens("'abc'", &d).unwrap(), "abc");
        assert!(replace_tokens("{NOPE}", &d).is_err());
    }

    #[test]
    fn metadata_parse() {
        let xml = "<versions><version>1.20.1-47.2.0</version><version>1.20.1-47.10.1</version><version>1.20-46.0.1</version><version>1.7.10-10.13.4.1614-1.7.10</version></versions>";
        let v = forge_from_metadata("1.20.1", xml);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].version, "47.10.1");
        let v = forge_from_metadata("1.7.10", xml);
        assert_eq!(v[0].version, "10.13.4.1614");
    }
}
