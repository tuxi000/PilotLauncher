//! 实例中的本地资源管理：mods / resourcepacks / shaderpacks / saves。
//!
//! mod 元数据依次尝试：fabric.mod.json → quilt.mod.json → META-INF/neoforge.mods.toml → META-INF/mods.toml → mcmod.info，
//! 读出名称、版本、简介、作者与图标（图标转为 data URL）。启用 / 禁用通过 `.disabled` 后缀实现。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::Value;

use super::{Platform, ResourceIndex, ResourceKind};
use crate::state::Core;

const ICON_LIMIT: u64 = 256 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocalResource {
    /// 文件或文件夹名（可能带 .disabled）
    pub file: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Option<String>,
    pub mod_id: Option<String>,
    /// fabric / quilt / forge / neoforge
    pub loader: Option<String>,
    /// 内嵌图标（data URL）
    pub icon: Option<String>,
    /// 来自安装记录的网络图标
    pub icon_url: Option<String>,
    pub enabled: bool,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub platform: Option<Platform>,
    pub project_id: Option<String>,
}

pub fn sub_dir(kind: ResourceKind) -> anyhow::Result<&'static str> {
    Ok(match kind {
        ResourceKind::Mod => "mods",
        ResourceKind::Resourcepack => "resourcepacks",
        ResourceKind::Shaderpack => "shaderpacks",
        ResourceKind::World => "saves",
        _ => bail!("不支持管理该类型的资源"),
    })
}

fn check_name(file: &str) -> anyhow::Result<()> {
    if file.is_empty() || file.contains('/') || file.contains('\\') || file.contains("..") || file.contains(':') {
        bail!("非法的文件名：{file}");
    }
    Ok(())
}

fn mtime(p: &Path) -> u64 {
    p.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn data_url(name: &str, bytes: &[u8]) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

type Zip = zip::ZipArchive<std::fs::File>;

fn zip_bytes(zip: &mut Zip, name: &str, limit: u64) -> Option<Vec<u8>> {
    let name = name.trim_start_matches('/').trim_start_matches("./");
    let mut e = zip.by_name(name).ok()?;
    if e.size() > limit {
        return None;
    }
    let mut b = Vec::new();
    e.read_to_end(&mut b).ok()?;
    Some(b)
}

fn zip_icon(zip: &mut Zip, name: &str) -> Option<String> {
    let b = zip_bytes(zip, name, ICON_LIMIT)?;
    data_url(name, &b)
}

/// 宽松解析 JSON：部分 mod 的 JSON 字符串里有裸换行
fn loose_json(bytes: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text)
        .ok()
        .or_else(|| serde_json::from_str(&text.replace(['\n', '\r', '\t'], " ")).ok())
}

fn text_of(v: &Value) -> Option<String> {
    let raw = match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => {
            let s: String = a.iter().filter_map(text_of).collect::<Vec<_>>().join("");
            Some(s)
        }
        Value::Object(o) => {
            let mut s = o.get("text").and_then(text_of).unwrap_or_default();
            if let Some(extra) = o.get("extra").and_then(text_of) {
                s.push_str(&extra);
            }
            Some(s)
        }
        _ => None,
    };
    raw.map(|s| strip_formatting(&s)).filter(|s| !s.trim().is_empty())
}

/// 去掉 § 格式代码
fn strip_formatting(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
            continue;
        }
        if c == '§' {
            skip = true;
            continue;
        }
        out.push(c);
    }
    out
}

fn authors_of(v: Option<&Value>) -> Option<String> {
    let a = v?.as_array()?;
    let names: Vec<String> = a
        .iter()
        .filter_map(|x| match x {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => o.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()),
            _ => None,
        })
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// fabric.mod.json 的 icon：字符串或 {尺寸: 路径}
fn icon_path(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o
            .iter()
            .max_by_key(|(k, _)| k.parse::<u32>().unwrap_or(0))
            .and_then(|(_, p)| p.as_str().map(|s| s.to_string())),
        _ => None,
    }
}

/// 极简 TOML 读取：只取全局键与第一个 [[mods]] 块中的字符串值
pub fn parse_mods_toml(text: &str) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut global = HashMap::new();
    let mut first = HashMap::new();
    let mut section = 0; // 0 全局，1 第一个 [[mods]]，2 其他
    let mut mods_seen = 0;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("[[") {
            let name = line.trim_start_matches("[[").split("]]").next().unwrap_or("").trim();
            if name == "mods" {
                mods_seen += 1;
                section = if mods_seen == 1 { 1 } else { 2 };
            } else {
                section = 2;
            }
            continue;
        }
        if line.starts_with('[') {
            section = 2;
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let key = k.trim().trim_matches('"').to_string();
        let v = v.trim();
        let value = if v.starts_with("\"\"\"") || v.starts_with("'''") {
            let quote = &v[..3];
            let mut buf = v[3..].to_string();
            if let Some(end) = buf.find(quote) {
                buf.truncate(end);
            } else {
                while i < lines.len() {
                    let l = lines[i];
                    i += 1;
                    if let Some(end) = l.find(quote) {
                        buf.push('\n');
                        buf.push_str(&l[..end]);
                        break;
                    }
                    buf.push('\n');
                    buf.push_str(l);
                }
            }
            buf.trim().to_string()
        } else if let Some(rest) = v.strip_prefix('"') {
            let mut out = String::new();
            let mut esc = false;
            for c in rest.chars() {
                if esc {
                    out.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    break;
                } else {
                    out.push(c);
                }
            }
            out
        } else if let Some(rest) = v.strip_prefix('\'') {
            rest.split('\'').next().unwrap_or("").to_string()
        } else {
            v.split('#').next().unwrap_or("").trim().to_string()
        };
        match section {
            0 => {
                global.insert(key, value);
            }
            1 => {
                first.insert(key, value);
            }
            _ => {}
        }
    }
    (global, first)
}

fn manifest_version(zip: &mut Zip) -> Option<String> {
    let b = zip_bytes(zip, "META-INF/MANIFEST.MF", 512 * 1024)?;
    let t = String::from_utf8_lossy(&b);
    t.lines()
        .find_map(|l| l.strip_prefix("Implementation-Version:").map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
}

fn read_mod(path: &Path, r: &mut LocalResource) {
    let Ok(f) = std::fs::File::open(path) else { return };
    let Ok(mut zip) = zip::ZipArchive::new(f) else { return };

    if let Some(v) = zip_bytes(&mut zip, "fabric.mod.json", 1024 * 1024).and_then(|b| loose_json(&b)) {
        r.loader = Some("fabric".into());
        r.mod_id = v.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
        r.name = v.get("name").and_then(|x| x.as_str()).map(|s| s.to_string()).or(r.mod_id.clone()).unwrap_or_default();
        r.version = v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string());
        r.description = v.get("description").and_then(|x| x.as_str()).map(|s| s.to_string());
        r.authors = authors_of(v.get("authors"));
        r.icon = icon_path(v.get("icon")).and_then(|p| zip_icon(&mut zip, &p));
        return;
    }
    if let Some(v) = zip_bytes(&mut zip, "quilt.mod.json", 1024 * 1024).and_then(|b| loose_json(&b)) {
        let q = v.get("quilt_loader").cloned().unwrap_or_default();
        let meta = q.get("metadata").cloned().unwrap_or_default();
        r.loader = Some("quilt".into());
        r.mod_id = q.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
        r.name = meta.get("name").and_then(|x| x.as_str()).map(|s| s.to_string()).or(r.mod_id.clone()).unwrap_or_default();
        r.version = q.get("version").and_then(|x| x.as_str()).map(|s| s.to_string());
        r.description = meta.get("description").and_then(|x| x.as_str()).map(|s| s.to_string());
        r.authors = meta
            .get("contributors")
            .and_then(|c| c.as_object())
            .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
            .filter(|s| !s.is_empty());
        r.icon = icon_path(meta.get("icon")).and_then(|p| zip_icon(&mut zip, &p));
        return;
    }
    for (file, loader) in [("META-INF/neoforge.mods.toml", "neoforge"), ("META-INF/mods.toml", "forge")] {
        if let Some(b) = zip_bytes(&mut zip, file, 1024 * 1024) {
            let text = String::from_utf8_lossy(&b).to_string();
            let (global, m) = parse_mods_toml(&text);
            r.loader = Some(loader.into());
            r.mod_id = m.get("modId").cloned();
            r.name = m.get("displayName").cloned().or(r.mod_id.clone()).unwrap_or_default();
            let mut ver = m.get("version").cloned();
            if ver.as_deref().map(|v| v.contains("${")).unwrap_or(false) {
                ver = manifest_version(&mut zip).or(ver);
            }
            r.version = ver;
            r.description = m.get("description").cloned();
            r.authors = m.get("authors").cloned().or_else(|| global.get("authors").cloned());
            let logo = m.get("logoFile").cloned().or_else(|| global.get("logoFile").cloned());
            r.icon = logo.and_then(|p| zip_icon(&mut zip, &p));
            return;
        }
    }
    if let Some(v) = zip_bytes(&mut zip, "mcmod.info", 1024 * 1024).and_then(|b| loose_json(&b)) {
        let first = match &v {
            Value::Array(a) => a.first().cloned(),
            Value::Object(o) => o.get("modList").and_then(|l| l.as_array()).and_then(|a| a.first().cloned()),
            _ => None,
        };
        if let Some(m) = first {
            r.loader = Some("forge".into());
            r.mod_id = m.get("modid").and_then(|x| x.as_str()).map(|s| s.to_string());
            r.name = m.get("name").and_then(|x| x.as_str()).map(|s| s.to_string()).or(r.mod_id.clone()).unwrap_or_default();
            r.version = m.get("version").and_then(|x| x.as_str()).map(|s| s.to_string());
            r.description = m.get("description").and_then(|x| x.as_str()).map(|s| s.to_string());
            r.authors = authors_of(m.get("authorList").or_else(|| m.get("authors")));
            r.icon = m
                .get("logoFile")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .and_then(|p| zip_icon(&mut zip, p));
        }
    }
}

fn read_pack_meta(path: &Path, is_dir: bool, r: &mut LocalResource) {
    if is_dir {
        if let Ok(b) = std::fs::read(path.join("pack.mcmeta")) {
            if let Some(v) = loose_json(&b) {
                r.description = v.pointer("/pack/description").and_then(text_of);
            }
        }
        let icon = path.join("pack.png");
        if icon.metadata().map(|m| m.len() <= ICON_LIMIT).unwrap_or(false) {
            if let Ok(b) = std::fs::read(&icon) {
                r.icon = data_url("pack.png", &b);
            }
        }
        return;
    }
    let Ok(f) = std::fs::File::open(path) else { return };
    let Ok(mut zip) = zip::ZipArchive::new(f) else { return };
    if let Some(v) = zip_bytes(&mut zip, "pack.mcmeta", 256 * 1024).and_then(|b| loose_json(&b)) {
        r.description = v.pointer("/pack/description").and_then(text_of);
    }
    r.icon = zip_icon(&mut zip, "pack.png");
}

fn display_name(file: &str) -> String {
    let base = file.strip_suffix(".disabled").unwrap_or(file);
    for ext in [".jar", ".zip"] {
        if let Some(b) = base.strip_suffix(ext) {
            return b.to_string();
        }
    }
    base.to_string()
}

pub fn list(core: &Core, instance: &str, kind: ResourceKind) -> anyhow::Result<Vec<LocalResource>> {
    let inst_dir = core.game_dirs().instance_dir(instance);
    if !inst_dir.is_dir() {
        bail!("实例 {instance} 不存在");
    }
    let sub = sub_dir(kind)?;
    let dir = inst_dir.join(sub);
    let index = ResourceIndex::load(&inst_dir);
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(out) };
    for e in rd.filter_map(|e| e.ok()) {
        let path = e.path();
        let file = e.file_name().to_string_lossy().to_string();
        let lower = file.to_ascii_lowercase();
        let is_dir = path.is_dir();
        let enabled = !lower.ends_with(".disabled");
        let base = lower.trim_end_matches(".disabled");
        let accept = match kind {
            ResourceKind::Mod => !is_dir && (base.ends_with(".jar") || base.ends_with(".zip") || base.ends_with(".litemod")),
            ResourceKind::Resourcepack | ResourceKind::Shaderpack => is_dir || base.ends_with(".zip"),
            ResourceKind::World => is_dir && path.join("level.dat").is_file(),
            _ => false,
        };
        if !accept {
            continue;
        }
        let mut r = LocalResource {
            name: display_name(&file),
            file: file.clone(),
            enabled,
            is_dir,
            size: if is_dir { 0 } else { e.metadata().map(|m| m.len()).unwrap_or(0) },
            modified: mtime(&path),
            ..Default::default()
        };
        match kind {
            ResourceKind::Mod => read_mod(&path, &mut r),
            ResourceKind::Resourcepack => read_pack_meta(&path, is_dir, &mut r),
            ResourceKind::World => {
                r.modified = mtime(&path.join("level.dat"));
                let icon = path.join("icon.png");
                if icon.metadata().map(|m| m.len() <= ICON_LIMIT).unwrap_or(false) {
                    if let Ok(b) = std::fs::read(&icon) {
                        r.icon = data_url("icon.png", &b);
                    }
                }
            }
            _ => {}
        }
        if r.name.trim().is_empty() {
            r.name = display_name(&file);
        }
        if let Some(entry) = index.find_by_path(&format!("{sub}/{file}")) {
            r.platform = Some(entry.platform);
            r.project_id = Some(entry.project_id.clone());
            r.icon_url = entry.icon_url.clone();
            if r.icon.is_none() && kind != ResourceKind::Mod {
                r.name = entry.title.clone();
            }
        }
        out.push(r);
    }
    out.sort_by(|a, b| {
        if kind == ResourceKind::World {
            b.modified.cmp(&a.modified)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });
    Ok(out)
}

fn resource_path(core: &Core, instance: &str, kind: ResourceKind, file: &str) -> anyhow::Result<PathBuf> {
    check_name(file)?;
    let p = core.game_dirs().instance_dir(instance).join(sub_dir(kind)?).join(file);
    if !p.exists() {
        bail!("文件不存在：{file}");
    }
    Ok(p)
}

/// 启用 / 禁用（.disabled 后缀），返回新的文件名
pub fn set_enabled(core: &Core, instance: &str, kind: ResourceKind, file: &str, enabled: bool) -> anyhow::Result<String> {
    if kind == ResourceKind::World {
        bail!("存档不支持禁用");
    }
    if core.is_running(instance) {
        bail!("游戏正在运行，请先关闭游戏");
    }
    let p = resource_path(core, instance, kind, file)?;
    let is_disabled = file.to_ascii_lowercase().ends_with(".disabled");
    let new_name = match (enabled, is_disabled) {
        (true, true) => file[..file.len() - ".disabled".len()].to_string(),
        (false, false) => format!("{file}.disabled"),
        _ => return Ok(file.to_string()),
    };
    let target = p.with_file_name(&new_name);
    if target.exists() {
        bail!("已存在同名文件 {new_name}");
    }
    std::fs::rename(&p, &target).context("重命名失败（文件可能被占用）")?;
    Ok(new_name)
}

pub fn delete(core: &Core, instance: &str, kind: ResourceKind, file: &str) -> anyhow::Result<()> {
    if core.is_running(instance) {
        bail!("游戏正在运行，请先关闭游戏");
    }
    let p = resource_path(core, instance, kind, file)?;
    if p.is_dir() {
        std::fs::remove_dir_all(&p).context("删除失败（文件可能被占用）")?;
    } else {
        std::fs::remove_file(&p).context("删除失败（文件可能被占用）")?;
    }
    let inst_dir = core.game_dirs().instance_dir(instance);
    let mut index = ResourceIndex::load(&inst_dir);
    let rel = format!("{}/{}", sub_dir(kind)?, file.trim_end_matches(".disabled"));
    let before = index.entries.len();
    index.entries.retain(|e| !e.path.eq_ignore_ascii_case(&rel));
    if index.entries.len() != before {
        index.save(&inst_dir)?;
    }
    Ok(())
}

/// 在资源管理器中定位
pub fn reveal(core: &Core, instance: &str, kind: ResourceKind, file: &str) -> anyhow::Result<()> {
    let p = resource_path(core, instance, kind, file)?;
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("/select,\"{}\"", p.display()));
    }
    #[cfg(not(windows))]
    {
        cmd.arg(p.parent().unwrap_or(&p));
    }
    cmd.spawn().context("无法打开资源管理器")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml() {
        let t = r#"
modLoader="javafml"
logoFile="logo.png"
[[mods]]
modId="examplemod"
version="${file.jarVersion}" # comment
displayName='Example Mod'
description='''
Line 1
Line 2'''
[[dependencies.examplemod]]
modId="forge"
"#;
        let (g, m) = parse_mods_toml(t);
        assert_eq!(g.get("logoFile").unwrap(), "logo.png");
        assert_eq!(m.get("modId").unwrap(), "examplemod");
        assert_eq!(m.get("displayName").unwrap(), "Example Mod");
        assert_eq!(m.get("version").unwrap(), "${file.jarVersion}");
        assert!(m.get("description").unwrap().contains("Line 2"));
    }

    #[test]
    fn names() {
        assert_eq!(display_name("sodium-0.5.jar.disabled"), "sodium-0.5");
        assert_eq!(strip_formatting("§aHello§r"), "Hello");
    }
}
