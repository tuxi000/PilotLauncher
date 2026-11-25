//! 暴露给前端的 Tauri 命令。前端通过 `invoke('命令名', { 参数（camelCase） })` 调用。

use std::sync::Arc;

use anyhow::{anyhow, bail};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use tauri::State;

use crate::auth::offline::{offline_uuid, validate_offline_name};
use crate::auth::store::{Account, AccountKind, AccountView};
use crate::auth::yggdrasil::{self, GameProfile, ServerInfo};
use crate::community::local::LocalResource;
use crate::community::modpack::{ArchiveInfo, MissingFile};
use crate::community::{
    self, Category, InstallItem, PlanItem, Platform, ProjectDetail, ResourceKind, ResourceVersion, SearchPage,
    SearchQuery,
};
use crate::instance::{self, InstanceEdit, InstanceView};
use crate::java::{detect, JavaInfo};
use crate::launch;
use crate::minecraft::manifest::{get_manifest, VersionManifest};
use crate::modloader::{self, LoaderKind, LoaderSpec, LoaderVersion};
use crate::settings::Settings;
use crate::state::Core;
use crate::system;
use crate::tasks::TaskSnapshot;
use crate::terracotta::{self, TerracottaView};
use crate::util::{now_ms, CmdResult, IntoCmd};

type AppState<'a> = State<'a, Arc<Core>>;

// ───────────────────────── 窗口 ─────────────────────────

/// 应用 Windows 11 窗口材质（Mica / Acrylic / Tabbed）
pub fn apply_window_effect(win: &tauri::WebviewWindow, effect: &str, theme: &str) -> anyhow::Result<()> {
    use tauri::window::{Effect, EffectsBuilder};
    let e = match (effect, theme) {
        ("mica", "dark") => Some(Effect::MicaDark),
        ("mica", "light") => Some(Effect::MicaLight),
        ("mica", _) => Some(Effect::Mica),
        ("tabbed", "dark") => Some(Effect::TabbedDark),
        ("tabbed", "light") => Some(Effect::TabbedLight),
        ("tabbed", _) => Some(Effect::Tabbed),
        ("acrylic", _) => Some(Effect::Acrylic),
        _ => None,
    };
    let cfg = e.map(|e| EffectsBuilder::new().effect(e).build());
    win.set_effects(cfg).map_err(|e| anyhow!("设置窗口材质失败：{e}"))
}

// ───────────────────────── 设置 / 系统 ─────────────────────────

#[tauri::command]
pub async fn get_settings(core: AppState<'_>) -> CmdResult<Settings> {
    Ok(core.settings())
}

#[tauri::command]
pub async fn save_settings(core: AppState<'_>, settings: Settings) -> CmdResult<Settings> {
    let old = core.settings();
    let saved = core.save_settings(settings).cmd()?;
    if old.window_effect != saved.window_effect || old.theme != saved.theme {
        if let Some(win) = core.main_window() {
            if let Err(e) = apply_window_effect(&win, &saved.window_effect, &saved.theme) {
                eprintln!("{e:#}");
            }
        }
    }
    if old.game_dir != saved.game_dir {
        if let Some(dir) = &saved.game_dir {
            std::fs::create_dir_all(dir).map_err(|e| format!("无法创建游戏目录：{e}"))?;
        }
    }
    Ok(saved)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub version: String,
    pub data_dir: String,
    pub game_dir: String,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub auto_memory_mb: u32,
    pub curseforge_configured: bool,
    pub local_config_path: Option<String>,
    /// 旧默认目录（%APPDATA%）里还有版本、且当前使用的是新默认目录时给出，用于提示迁移
    pub legacy_game_dir: Option<String>,
}

#[tauri::command]
pub async fn get_system_info(core: AppState<'_>) -> CmdResult<SystemInfo> {
    Ok(SystemInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: core.paths.data_dir.display().to_string(),
        game_dir: core.game_dirs().root.display().to_string(),
        total_memory_mb: system::total_memory_mb(),
        available_memory_mb: system::available_memory_mb(),
        auto_memory_mb: system::auto_memory_mb(),
        curseforge_configured: core.local_config.curseforge_api_key.is_some(),
        local_config_path: core.local_config.loaded_from.clone(),
        legacy_game_dir: {
            let legacy = core.paths.legacy_game_dir();
            let has_versions = std::fs::read_dir(legacy.join("versions"))
                .map(|mut d| d.next().is_some())
                .unwrap_or(false);
            (core.settings().game_dir.is_none() && core.game_dirs().root != legacy && has_versions)
                .then(|| legacy.display().to_string())
        },
    })
}

/// 实时内存状态（设置页 / 实例设置的内存条每秒轮询）
#[tauri::command]
pub async fn get_memory_status() -> CmdResult<system::MemoryStatus> {
    Ok(system::memory_status())
}

// ───────────────────────── 版本 / 实例 ─────────────────────────

#[tauri::command]
pub async fn get_version_manifest(core: AppState<'_>, force: Option<bool>) -> CmdResult<VersionManifest> {
    get_manifest(&core, force.unwrap_or(false)).await.cmd()
}

#[tauri::command]
pub async fn list_instances(core: AppState<'_>) -> CmdResult<Vec<InstanceView>> {
    Ok(instance::list(&core))
}

#[tauri::command]
pub async fn create_instance(
    core: AppState<'_>,
    name: String,
    mc_version: String,
    loaders: Option<Vec<LoaderSpec>>,
    addons: Option<Vec<InstallItem>>,
) -> CmdResult<String> {
    instance::create(&core, name, mc_version, loaders.unwrap_or_default(), addons.unwrap_or_default()).cmd()
}

#[tauri::command]
pub async fn rename_instance(core: AppState<'_>, old_name: String, new_name: String) -> CmdResult<()> {
    instance::rename(&core, &old_name, &new_name).cmd()
}

#[tauri::command]
pub async fn delete_instance(core: AppState<'_>, name: String) -> CmdResult<()> {
    instance::delete(&core, &name).cmd()
}

#[tauri::command]
pub async fn update_instance(core: AppState<'_>, name: String, edit: InstanceEdit) -> CmdResult<InstanceView> {
    instance::update(&core, &name, edit).cmd()
}

#[tauri::command]
pub async fn repair_instance(core: AppState<'_>, name: String) -> CmdResult<String> {
    instance::repair(&core, name).cmd()
}

#[tauri::command]
pub async fn select_instance(core: AppState<'_>, name: Option<String>) -> CmdResult<Settings> {
    core.update_settings(|s| s.selected_instance = name).cmd()
}

#[tauri::command]
pub async fn list_saves(core: AppState<'_>, name: String) -> CmdResult<Vec<String>> {
    Ok(instance::list_saves(&core, &name))
}

/// 打开实例目录（versions/<实例名>/）；`sub` 为子目录（资源管理里的“打开 mods 文件夹”等）
#[tauri::command]
pub async fn open_instance_dir(core: AppState<'_>, name: String, sub: Option<String>) -> CmdResult<()> {
    let mut dir = core.game_dirs().instance_dir(&name);
    if let Some(s) = sub.filter(|s| !s.is_empty()) {
        if s.contains("..") || s.contains(':') {
            return Err("非法的子目录".into());
        }
        dir = dir.join(s);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open_in_explorer(&dir.display().to_string())
}

// ───────────────────────── 任务 ─────────────────────────

#[tauri::command]
pub async fn list_tasks(core: AppState<'_>) -> CmdResult<Vec<TaskSnapshot>> {
    Ok(core.tasks.list())
}

#[tauri::command]
pub async fn cancel_task(core: AppState<'_>, id: String) -> CmdResult<()> {
    match core.tasks.get(&id) {
        Some(t) => {
            t.cancel();
            Ok(())
        }
        None => Err("任务不存在".into()),
    }
}

#[tauri::command]
pub async fn clear_finished_tasks(core: AppState<'_>) -> CmdResult<Vec<TaskSnapshot>> {
    core.tasks.clear_finished();
    Ok(core.tasks.list())
}

// ───────────────────────── 账户 ─────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountList {
    pub accounts: Vec<AccountView>,
    pub selected: Option<String>,
}

#[tauri::command]
pub async fn list_accounts(core: AppState<'_>) -> CmdResult<AccountList> {
    let accounts = core.accounts.lock().await.views();
    Ok(AccountList { accounts, selected: core.settings().selected_account })
}

#[tauri::command]
pub async fn add_offline_account(core: AppState<'_>, name: String) -> CmdResult<AccountView> {
    let name = name.trim().to_string();
    validate_offline_name(&name).cmd()?;
    let account = Account {
        id: uuid::Uuid::new_v4().simple().to_string(),
        kind: AccountKind::Offline,
        uuid: offline_uuid(&name),
        name,
        access_token: uuid::Uuid::new_v4().simple().to_string(),
        client_token: String::new(),
        server_url: None,
        server_name: None,
        username: None,
        metadata_b64: None,
        created_at: now_ms(),
    };
    let saved = core.accounts.lock().await.upsert(account).cmd()?;
    let id = saved.id.clone();
    core.update_settings(|s| s.selected_account = Some(id)).cmd()?;
    Ok(AccountView::from(&saved))
}

#[tauri::command]
pub async fn check_auth_server(core: AppState<'_>, url: String) -> CmdResult<ServerInfo> {
    yggdrasil::resolve_server(&core.http, &url).await.cmd()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YggLoginResult {
    /// ok / choose_profile
    pub status: String,
    pub account: Option<AccountView>,
    pub profiles: Vec<GameProfile>,
}

async fn yggdrasil_login(
    core: &Core,
    server: &str,
    username: &str,
    password: &str,
    profile_id: Option<String>,
) -> anyhow::Result<YggLoginResult> {
    let info = yggdrasil::resolve_server(&core.http, server).await?;
    let root = info.api_root.clone();
    let token = uuid::Uuid::new_v4().simple().to_string();
    let r = yggdrasil::authenticate(&core.http, &root, username, password, &token).await?;
    let client_token = r.client_token.clone().unwrap_or(token);

    let (profile, access_token) = match r.selected_profile.clone() {
        Some(p) if profile_id.as_deref().map(|id| id == p.id).unwrap_or(true) => (p, r.access_token.clone()),
        _ => {
            if r.available_profiles.is_empty() {
                bail!("该账户下还没有角色，请先到认证服务器网站创建角色");
            }
            let chosen = match &profile_id {
                Some(id) => r.available_profiles.iter().find(|p| &p.id == id).cloned(),
                None if r.available_profiles.len() == 1 => r.available_profiles.first().cloned(),
                None => None,
            };
            match chosen {
                Some(p) => {
                    let rr = yggdrasil::refresh(&core.http, &root, &r.access_token, &client_token, Some(&p)).await?;
                    (rr.selected_profile.unwrap_or(p), rr.access_token)
                }
                None => {
                    return Ok(YggLoginResult {
                        status: "choose_profile".into(),
                        account: None,
                        profiles: r.available_profiles.clone(),
                    })
                }
            }
        }
    };

    let account = Account {
        id: uuid::Uuid::new_v4().simple().to_string(),
        kind: AccountKind::Yggdrasil,
        name: profile.name,
        uuid: profile.id,
        access_token,
        client_token,
        server_url: Some(root),
        server_name: info.server_name.clone(),
        username: Some(username.to_string()),
        metadata_b64: Some(info.metadata_b64.clone()),
        created_at: now_ms(),
    };
    let saved = core.accounts.lock().await.upsert(account)?;
    let id = saved.id.clone();
    core.update_settings(|s| s.selected_account = Some(id))?;
    Ok(YggLoginResult {
        status: "ok".into(),
        account: Some(AccountView::from(&saved)),
        profiles: Vec::new(),
    })
}

#[tauri::command]
pub async fn add_yggdrasil_account(
    core: AppState<'_>,
    server: String,
    username: String,
    password: String,
    profile_id: Option<String>,
) -> CmdResult<YggLoginResult> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("请输入账号和密码".into());
    }
    yggdrasil_login(&core, &server, username.trim(), &password, profile_id).await.cmd()
}

#[tauri::command]
pub async fn remove_account(core: AppState<'_>, id: String) -> CmdResult<AccountList> {
    let mut store = core.accounts.lock().await;
    store.remove(&id).cmd()?;
    let next = store.accounts.first().map(|a| a.id.clone());
    let accounts = store.views();
    drop(store);
    let selected = if core.settings().selected_account.as_deref() == Some(id.as_str()) {
        core.update_settings(|s| s.selected_account = next).cmd()?.selected_account
    } else {
        core.settings().selected_account
    };
    Ok(AccountList { accounts, selected })
}

#[tauri::command]
pub async fn select_account(core: AppState<'_>, id: String) -> CmdResult<Settings> {
    if core.accounts.lock().await.get(&id).is_none() {
        return Err("账户不存在".into());
    }
    core.update_settings(|s| s.selected_account = Some(id)).cmd()
}

// ───────────────────────── Java ─────────────────────────

#[tauri::command]
pub async fn list_java(core: AppState<'_>) -> CmdResult<Vec<JavaInfo>> {
    let paths = core.paths.clone();
    tokio::task::spawn_blocking(move || detect::scan(&paths))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inspect_java(path: String) -> CmdResult<JavaInfo> {
    detect::inspect(&path, "自定义").ok_or_else(|| "找不到 javaw.exe / java.exe".to_string())
}

// ───────────────────────── 启动 ─────────────────────────

#[tauri::command]
pub async fn launch_game(core: AppState<'_>, name: String) -> CmdResult<()> {
    let core = core.inner().clone();
    launch::launch(core, name).await.cmd()
}

#[tauri::command]
pub async fn kill_game(core: AppState<'_>, name: String) -> CmdResult<()> {
    launch::kill(&core, &name).cmd()
}

#[tauri::command]
pub async fn get_game_log(core: AppState<'_>, name: String) -> CmdResult<Vec<String>> {
    Ok(launch::get_log(&core, &name))
}

#[tauri::command]
pub async fn running_games(core: AppState<'_>) -> CmdResult<Vec<String>> {
    Ok(core.running_instances())
}

// ───────────────────────── 杂项 ─────────────────────────

fn open_in_explorer(target: &str) -> CmdResult<()> {
    std::process::Command::new("explorer")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开：{e}"))
}

#[tauri::command]
pub async fn open_path(path: String) -> CmdResult<()> {
    if !std::path::Path::new(&path).exists() {
        return Err("路径不存在".into());
    }
    open_in_explorer(&path)
}

#[tauri::command]
pub async fn open_url(url: String) -> CmdResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("只能打开 http(s) 链接".into());
    }
    open_in_explorer(&url)
}

/// 读取本地图片并转为 data URL（用于自定义背景，无需开启 asset 协议）
#[tauri::command]
pub async fn read_image_data_url(path: String) -> CmdResult<String> {
    let p = std::path::PathBuf::from(path.trim().trim_matches('"'));
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => return Err("只支持 png / jpg / webp / gif / bmp 图片".into()),
    };
    let meta = tokio::fs::metadata(&p).await.map_err(|_| "图片不存在".to_string())?;
    if meta.len() > 25 * 1024 * 1024 {
        return Err("图片不能超过 25 MB".into());
    }
    let bytes = tokio::fs::read(&p).await.map_err(|e| e.to_string())?;
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

// ───────────────────────── 模组加载器 ─────────────────────────

#[tauri::command]
pub async fn list_loader_versions(
    core: AppState<'_>,
    kind: LoaderKind,
    mc_version: String,
    force: Option<bool>,
) -> CmdResult<Vec<LoaderVersion>> {
    modloader::list_versions(&core, kind, &mc_version, force.unwrap_or(false)).await.cmd()
}

/// 为已有实例安装 / 更换加载器，返回任务 id。`addons` 为随加载器安装的前置模组（Fabric API / QFAPI）
#[tauri::command]
pub async fn install_loader(
    core: AppState<'_>,
    name: String,
    loaders: Vec<LoaderSpec>,
    addons: Option<Vec<InstallItem>>,
) -> CmdResult<String> {
    modloader::start_install(core.inner(), name, loaders, addons.unwrap_or_default()).cmd()
}

// ───────────────────────── 社区资源 ─────────────────────────

#[tauri::command]
pub async fn community_search(core: AppState<'_>, query: SearchQuery) -> CmdResult<SearchPage> {
    community::search(&core, &query).await.cmd()
}

#[tauri::command]
pub async fn community_categories(
    core: AppState<'_>,
    platform: Platform,
    kind: ResourceKind,
) -> CmdResult<Vec<Category>> {
    community::categories(&core, platform, kind).await.cmd()
}

#[tauri::command]
pub async fn community_project(
    core: AppState<'_>,
    platform: Platform,
    id: String,
    kind: ResourceKind,
) -> CmdResult<ProjectDetail> {
    community::project(&core, platform, &id, kind).await.cmd()
}

#[tauri::command]
pub async fn community_versions(
    core: AppState<'_>,
    platform: Platform,
    project_id: String,
    mc_version: Option<String>,
    loaders: Option<Vec<String>>,
) -> CmdResult<Vec<ResourceVersion>> {
    let mc = mc_version.filter(|m| !m.trim().is_empty());
    community::versions(&core, platform, &project_id, mc.as_deref(), &loaders.unwrap_or_default())
        .await
        .cmd()
}

/// 安装前的计划：所选版本 + 必需依赖（已安装的会标记）
#[tauri::command]
pub async fn community_plan(
    core: AppState<'_>,
    instance: String,
    kind: ResourceKind,
    title: String,
    icon_url: Option<String>,
    version: ResourceVersion,
) -> CmdResult<Vec<PlanItem>> {
    community::plan_install(&core, &instance, kind, title, icon_url, version).await.cmd()
}

#[tauri::command]
pub async fn community_install(
    core: AppState<'_>,
    instance: String,
    kind: ResourceKind,
    save: Option<String>,
    items: Vec<InstallItem>,
) -> CmdResult<String> {
    community::start_install(core.inner(), instance, kind, save, items).cmd()
}

#[tauri::command]
pub async fn community_install_modpack(
    core: AppState<'_>,
    version: ResourceVersion,
    name: String,
    title: String,
) -> CmdResult<String> {
    community::modpack::start_remote(core.inner(), version, name, title).cmd()
}

/// 识别本地压缩包：mrpack / curseforge / world / unknown
#[tauri::command]
pub async fn inspect_archive(path: String) -> CmdResult<ArchiveInfo> {
    let p = std::path::PathBuf::from(path.trim().trim_matches('"'));
    if !p.is_file() {
        return Err("文件不存在".into());
    }
    tokio::task::spawn_blocking(move || community::modpack::inspect(&p))
        .await
        .map_err(|e| e.to_string())?
        .cmd()
}

#[tauri::command]
pub async fn import_modpack(core: AppState<'_>, path: String, name: String) -> CmdResult<String> {
    community::modpack::start_import(core.inner(), path, name).cmd()
}

#[tauri::command]
pub async fn import_world(core: AppState<'_>, path: String, instance: String) -> CmdResult<String> {
    community::world::start_import(core.inner(), path, instance).cmd()
}

#[tauri::command]
pub async fn get_missing_files(core: AppState<'_>, name: String) -> CmdResult<Vec<MissingFile>> {
    Ok(community::modpack::read_missing(&core, &name))
}

#[tauri::command]
pub async fn dismiss_missing_files(core: AppState<'_>, name: String) -> CmdResult<()> {
    community::modpack::dismiss_missing(&core, &name).cmd()
}

// ───────────────────────── 实例内资源管理 ─────────────────────────

#[tauri::command]
pub async fn list_instance_resources(
    core: AppState<'_>,
    name: String,
    kind: ResourceKind,
) -> CmdResult<Vec<LocalResource>> {
    let core = core.inner().clone();
    tokio::task::spawn_blocking(move || community::local::list(&core, &name, kind))
        .await
        .map_err(|e| e.to_string())?
        .cmd()
}

#[tauri::command]
pub async fn set_resource_enabled(
    core: AppState<'_>,
    name: String,
    kind: ResourceKind,
    file: String,
    enabled: bool,
) -> CmdResult<String> {
    community::local::set_enabled(&core, &name, kind, &file, enabled).cmd()
}

#[tauri::command]
pub async fn delete_instance_resource(
    core: AppState<'_>,
    name: String,
    kind: ResourceKind,
    file: String,
) -> CmdResult<()> {
    community::local::delete(&core, &name, kind, &file).cmd()
}

#[tauri::command]
pub async fn reveal_instance_resource(
    core: AppState<'_>,
    name: String,
    kind: ResourceKind,
    file: String,
) -> CmdResult<()> {
    community::local::reveal(&core, &name, kind, &file).cmd()
}

// ───────────────────────── 联机（Terracotta | 陶瓦联机） ─────────────────────────
//
// 所有操作都通过 Terracotta 的本机 HTTP 接口完成；状态变化另外通过 `terracotta-state` 事件推送。

#[tauri::command]
pub async fn terracotta_status(core: AppState<'_>) -> CmdResult<TerracottaView> {
    Ok(core.terracotta.view(&core))
}

/// 联机页打开 / 关闭（只在联机页打开或房间进行中时轮询）
#[tauri::command]
pub async fn terracotta_watch(core: AppState<'_>, watching: bool) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::watch(&core, watching).await;
    Ok(core.terracotta.view(&core))
}

/// 下载并安装联机组件，返回任务 id
#[tauri::command]
pub async fn terracotta_install(core: AppState<'_>) -> CmdResult<String> {
    terracotta::install::start_install(core.inner()).cmd()
}

#[tauri::command]
pub async fn terracotta_start(core: AppState<'_>) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::start(&core).await.cmd()?;
    Ok(core.terracotta.view(&core))
}

#[tauri::command]
pub async fn terracotta_stop(core: AppState<'_>) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::stop(&core).await.cmd()?;
    Ok(core.terracotta.view(&core))
}

#[tauri::command]
pub async fn terracotta_restart(core: AppState<'_>) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::restart(&core).await.cmd()?;
    Ok(core.terracotta.view(&core))
}

/// 作为房主：扫描本机局域网世界，扫描到后自动创建房间并生成邀请码
#[tauri::command]
pub async fn terracotta_host(core: AppState<'_>) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::host(&core).await.cmd()?;
    Ok(core.terracotta.view(&core))
}

/// 作为房客：用邀请码加入房间
#[tauri::command]
pub async fn terracotta_join(core: AppState<'_>, code: String) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::join(&core, &code).await.cmd()?;
    Ok(core.terracotta.view(&core))
}

/// 退出房间 / 关闭房间 / 清除异常
#[tauri::command]
pub async fn terracotta_leave(core: AppState<'_>) -> CmdResult<TerracottaView> {
    let core = core.inner().clone();
    terracotta::leave(&core).await.cmd()?;
    Ok(core.terracotta.view(&core))
}

/// 联机组件日志（最后 400 行，用于反馈问题）
#[tauri::command]
pub async fn terracotta_log(core: AppState<'_>) -> CmdResult<String> {
    let core = core.inner().clone();
    terracotta::log_tail(&core).await.cmd()
}
