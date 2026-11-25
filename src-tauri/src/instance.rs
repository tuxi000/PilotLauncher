//! 实例管理（版本隔离）：每个实例 = versions/<实例名>/，同时作为 --gameDir。

use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::minecraft::april_fools;
use crate::paths::GameDirs;
use crate::state::Core;
use crate::tasks::spawn_task;
use crate::util::{now_ms, read_json, validate_instance_name, write_json_atomic};

pub const META_FILE: &str = "glass-instance.json";
/// 自动图标：有加载器时显示加载器图标，否则按版本类型（前端解析；旧数据中的 "grass" 也按自动处理）
pub const AUTO_ICON: &str = "auto";
/// 愚人节版本在 InstanceView.version_type 中的取值（与前端 api.ts 的 VERSION_TYPE_LABEL 对应）
pub const APRIL_FOOLS_TYPE: &str = "april_fools";
/// 版本隔离：这些目录全部位于实例目录内
pub const SUBDIRS: &[&str] = &["mods", "saves", "resourcepacks", "shaderpacks", "config"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LoaderInfo {
    /// forge / neoforge / fabric / quilt / optifine
    pub kind: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceMeta {
    pub name: String,
    pub mc_version: String,
    pub loader: Option<LoaderInfo>,
    /// 与 Forge 共存时以 mod 形式安装的 OptiFine 版本（单独安装时 loader.kind = optifine）
    pub optifine: Option<String>,
    pub created_at: u64,
    pub last_played: Option<u64>,
    pub description: String,
    /// 图标名（前端内置图标；"auto" / 空 / 旧默认值 "grass" 表示自动）
    pub icon: String,
    /// 实例级 Java 覆盖
    pub java_path: Option<String>,
    /// 实例级内存覆盖（MB）
    pub memory_mb: Option<u32>,
    pub jvm_args: String,
    pub game_args: String,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    /// 原版文件是否安装完成
    pub installed: bool,
}

/// 前端可编辑的字段
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceEdit {
    pub description: String,
    pub icon: String,
    pub java_path: Option<String>,
    pub memory_mb: Option<u32>,
    pub jvm_args: String,
    pub game_args: String,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    #[serde(flatten)]
    pub meta: InstanceMeta,
    pub dir: String,
    pub version_type: String,
    /// 是否由 PilotLauncher 创建（存在 glass-instance.json）
    pub managed: bool,
    pub has_json: bool,
    pub has_jar: bool,
    pub running: bool,
}

pub fn meta_path(dirs: &GameDirs, name: &str) -> std::path::PathBuf {
    dirs.instance_dir(name).join(META_FILE)
}

/// 读取实例元数据；从其他启动器导入的版本没有元数据文件时，根据版本 JSON 推断。
pub fn load_meta(dirs: &GameDirs, name: &str) -> anyhow::Result<InstanceMeta> {
    let mp = meta_path(dirs, name);
    if mp.is_file() {
        let mut m: InstanceMeta = read_json(&mp)?;
        m.name = name.to_string();
        return Ok(m);
    }
    let json_path = dirs.version_json(name);
    if !json_path.is_file() {
        bail!("实例 {name} 不存在");
    }
    let v: serde_json::Value = read_json(&json_path)?;
    let mc_version = v
        .get("clientVersion")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("inheritsFrom").and_then(|x| x.as_str()))
        .or_else(|| v.get("id").and_then(|x| x.as_str()))
        .unwrap_or(name)
        .to_string();
    Ok(InstanceMeta {
        name: name.to_string(),
        mc_version,
        installed: true,
        icon: AUTO_ICON.into(),
        ..Default::default()
    })
}

pub fn save_meta(dirs: &GameDirs, meta: &InstanceMeta) -> anyhow::Result<()> {
    write_json_atomic(&meta_path(dirs, &meta.name), meta)
}

/// 版本类型：release / snapshot / old_beta / old_alpha，愚人节版本返回 "april_fools"（前端据此显示金块图标）。
/// 外部版本同样适用：名单按 mc_version（clientVersion / inheritsFrom / id）匹配，规则再看版本 JSON 的 type 与 releaseTime。
fn version_type_of(dirs: &GameDirs, name: &str, mc_version: &str) -> String {
    let json = read_json::<serde_json::Value>(&dirs.version_json(name)).ok();
    let str_of = |key: &str| -> String {
        json.as_ref()
            .and_then(|v| v.get(key))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    let mut t = str_of("type");
    if t.is_empty() {
        t = "release".into();
    }
    let release_time = str_of("releaseTime");
    let client = str_of("clientVersion");
    let id = if client.is_empty() { mc_version.to_string() } else { client };
    if april_fools::in_list(mc_version) || april_fools::is_april_fools(&id, &t, &release_time) {
        return APRIL_FOOLS_TYPE.into();
    }
    t
}

pub fn view(core: &Core, dirs: &GameDirs, name: &str) -> anyhow::Result<InstanceView> {
    let meta = load_meta(dirs, name)?;
    let dir = dirs.instance_dir(name);
    let jar_id = read_json::<serde_json::Value>(&dirs.version_json(name))
        .ok()
        .and_then(|v| {
            v.get("jar")
                .or_else(|| v.get("inheritsFrom"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| name.to_string());
    Ok(InstanceView {
        managed: meta_path(dirs, name).is_file(),
        has_json: dirs.version_json(name).is_file(),
        has_jar: dirs.version_jar(&jar_id).is_file(),
        version_type: version_type_of(dirs, name, &meta.mc_version),
        running: core.is_running(name),
        dir: dir.display().to_string(),
        meta,
    })
}

pub fn list(core: &Core) -> Vec<InstanceView> {
    let dirs = core.game_dirs();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dirs.versions()) else {
        return out;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let has_meta = meta_path(&dirs, &name).is_file();
        let has_json = dirs.version_json(&name).is_file();
        if !has_meta && !has_json {
            continue;
        }
        if let Ok(v) = view(core, &dirs, &name) {
            out.push(v);
        }
    }
    out.sort_by(|a, b| {
        b.meta
            .last_played
            .unwrap_or(0)
            .cmp(&a.meta.last_played.unwrap_or(0))
            .then_with(|| b.meta.created_at.cmp(&a.meta.created_at))
            .then_with(|| a.meta.name.cmp(&b.meta.name))
    });
    out
}

fn ensure_subdirs(dir: &Path) -> anyhow::Result<()> {
    for sub in SUBDIRS {
        std::fs::create_dir_all(dir.join(sub))?;
    }
    Ok(())
}

/// 创建实例目录与元数据（不安装文件）
pub fn prepare(core: &Core, name: &str, mc_version: &str) -> anyhow::Result<()> {
    validate_instance_name(name)?;
    let dirs = core.game_dirs();
    let dir = dirs.instance_dir(name);
    if dir.exists() {
        bail!("已存在同名实例或文件夹：{name}");
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("无法创建实例目录 {}", dir.display()))?;
    ensure_subdirs(&dir)?;
    let meta = InstanceMeta {
        name: name.to_string(),
        mc_version: mc_version.to_string(),
        created_at: now_ms(),
        icon: AUTO_ICON.into(),
        installed: false,
        ..Default::default()
    };
    save_meta(&dirs, &meta)?;
    if core.settings().selected_instance.is_none() {
        let n = name.to_string();
        core.update_settings(|s| s.selected_instance = Some(n))?;
    }
    Ok(())
}

/// 创建实例并在后台安装原版（以及可选的加载器），返回任务 id
pub fn create(
    core: &Arc<Core>,
    name: String,
    mc_version: String,
    loaders: Vec<crate::modloader::LoaderSpec>,
    addons: Vec<crate::community::InstallItem>,
) -> anyhow::Result<String> {
    use crate::modloader::{self, Stager};
    if !loaders.is_empty() && april_fools::in_list(&mc_version) {
        bail!("愚人节版本不支持模组加载器");
    }
    modloader::validate(&loaders)?;
    modloader::validate_addons(&loaders, &addons)?;
    prepare(core, &name, &mc_version)?;

    let label = if loaders.is_empty() {
        mc_version.clone()
    } else {
        let mut l = loaders
            .iter()
            .map(|s| format!("{} {}", s.kind.display_name(), modloader::display_version(s.kind, &mc_version, &s.version)))
            .collect::<Vec<_>>()
            .join(" + ");
        for a in &addons {
            l.push_str(&format!(" + {}", a.title));
        }
        format!("{mc_version} · {l}")
    };
    let task = core.tasks.create(format!("安装 {name}（{label}）"), "install");
    let id = task.id.clone();
    let core2 = core.clone();
    spawn_task(task, move |task| async move {
        if loaders.is_empty() {
            return crate::minecraft::install::install_vanilla(core2, task, mc_version, name).await;
        }
        let total = 3 + modloader::stage_count(&loaders) + modloader::addon_stages(&addons);
        crate::minecraft::install::install_vanilla_staged(&core2, &task, &mc_version, &name, 1, total).await?;
        let mut st = Stager::new(task.clone(), 3, total);
        modloader::install_loaders(&core2, &task, &name, &loaders, &mut st).await?;
        modloader::install_addons(&core2, &task, &name, addons, &mut st).await
    });
    Ok(id)
}

/// 补全 / 修复实例文件，返回任务 id
pub fn repair(core: &Arc<Core>, name: String) -> anyhow::Result<String> {
    let dirs = core.game_dirs();
    if !dirs.version_json(&name).is_file() {
        bail!("实例 {name} 缺少版本文件，无法修复。请删除后重新安装。");
    }
    if core.is_running(&name) {
        bail!("实例正在运行，请先关闭游戏");
    }
    let task = core.tasks.create(format!("修复 {name}"), "repair");
    let id = task.id.clone();
    let core2 = core.clone();
    spawn_task(task, move |task| async move {
        crate::minecraft::install::repair(core2, task, name).await
    });
    Ok(id)
}

pub fn rename(core: &Core, old: &str, new: &str) -> anyhow::Result<()> {
    validate_instance_name(new)?;
    if old == new {
        return Ok(());
    }
    if core.is_running(old) {
        bail!("实例正在运行，请先关闭游戏");
    }
    let dirs = core.game_dirs();
    let old_dir = dirs.instance_dir(old);
    let new_dir = dirs.instance_dir(new);
    if !old_dir.is_dir() {
        bail!("实例 {old} 不存在");
    }
    if new_dir.exists() && !old.eq_ignore_ascii_case(new) {
        bail!("已存在同名实例：{new}");
    }
    let mut meta = load_meta(&dirs, old)?;
    std::fs::rename(&old_dir, &new_dir).context("重命名实例目录失败（文件可能被占用）")?;

    for ext in ["json", "jar"] {
        let from = new_dir.join(format!("{old}.{ext}"));
        let to = new_dir.join(format!("{new}.{ext}"));
        if from.is_file() {
            std::fs::rename(&from, &to)?;
        }
    }
    let json_path = dirs.version_json(new);
    if json_path.is_file() {
        let mut v: serde_json::Value = read_json(&json_path)?;
        v["id"] = serde_json::Value::String(new.to_string());
        write_json_atomic(&json_path, &v)?;
    }
    meta.name = new.to_string();
    save_meta(&dirs, &meta)?;
    if core.settings().selected_instance.as_deref() == Some(old) {
        let n = new.to_string();
        core.update_settings(|s| s.selected_instance = Some(n))?;
    }
    Ok(())
}

pub fn delete(core: &Core, name: &str) -> anyhow::Result<()> {
    if core.is_running(name) {
        bail!("实例正在运行，请先关闭游戏");
    }
    let dirs = core.game_dirs();
    let dir = dirs.instance_dir(name);
    if !dir.is_dir() {
        bail!("实例 {name} 不存在");
    }
    std::fs::remove_dir_all(&dir).context("删除实例失败（文件可能被占用）")?;
    if core.settings().selected_instance.as_deref() == Some(name) {
        core.update_settings(|s| s.selected_instance = None)?;
    }
    Ok(())
}

pub fn update(core: &Core, name: &str, edit: InstanceEdit) -> anyhow::Result<InstanceView> {
    let dirs = core.game_dirs();
    let mut meta = load_meta(&dirs, name)?;
    meta.description = edit.description;
    if !edit.icon.is_empty() {
        meta.icon = edit.icon;
    }
    meta.java_path = edit.java_path.filter(|s| !s.trim().is_empty());
    meta.memory_mb = edit.memory_mb.filter(|m| *m >= 512);
    meta.jvm_args = edit.jvm_args;
    meta.game_args = edit.game_args;
    meta.window_width = edit.window_width.filter(|w| *w > 0);
    meta.window_height = edit.window_height.filter(|h| *h > 0);
    ensure_subdirs(&dirs.instance_dir(name))?;
    save_meta(&dirs, &meta)?;
    view(core, &dirs, name)
}

pub fn set_installed(dirs: &GameDirs, name: &str, installed: bool) -> anyhow::Result<()> {
    let mut meta = load_meta(dirs, name)?;
    meta.installed = installed;
    save_meta(dirs, &meta)
}

pub fn touch_last_played(dirs: &GameDirs, name: &str) -> anyhow::Result<()> {
    let mut meta = load_meta(dirs, name)?;
    meta.last_played = Some(now_ms());
    save_meta(dirs, &meta)
}

/// 列出实例中的存档（数据包、地图安装的目标选择）
pub fn list_saves(core: &Core, name: &str) -> Vec<String> {
    let dir = core.game_dirs().instance_dir(name).join("saves");
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}
