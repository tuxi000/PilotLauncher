//! 模组加载器安装：Fabric / Quilt / Forge / NeoForge / OptiFine。
//!
//! 设计约定：
//! - 安装加载器时，先取得「纯原版」版本 JSON（实例目录下的 `.glass-vanilla.json` 备份，没有则重新下载），
//!   再把加载器的 profile / version.json 作为子版本（inheritsFrom）**合并**进实例 JSON。
//!   这样实例 JSON 是自包含的（不依赖 versions/<原版>/ 目录），重装 / 更换加载器时也总是从原版重新合并。
//! - Forge / NeoForge 新版安装器（含 processors）共用 `forge::run_modern`；旧版（versionInfo + universal）走 `forge::run_legacy`。
//! - 所有 Java 进程（processors、OptiFine Patcher）都使用实例对应的 Java；没有时复用 Java 运行时自动下载。

pub mod fabric;
pub mod forge;
pub mod meta;
pub mod optifine;

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;

use crate::community::InstallItem;
use crate::download::{DownloadOptions, Verify};
use crate::instance::{self, LoaderInfo};
use crate::java::{detect, runtime, JavaInfo};
use crate::minecraft::install;
use crate::minecraft::version::load_resolved;
use crate::state::Core;
use crate::tasks::TaskHandle;
use crate::util::{read_json, write_json_atomic};

/// 实例目录中的原版 JSON 备份（安装 / 更换加载器时以它为基础重新合并）
pub const VANILLA_BACKUP: &str = ".glass-vanilla.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
    OptiFine,
}

impl LoaderKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            LoaderKind::Forge => "Forge",
            LoaderKind::NeoForge => "NeoForge",
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
            LoaderKind::OptiFine => "OptiFine",
        }
    }

    /// glass-instance.json 中记录的类型名
    pub fn id(&self) -> &'static str {
        match self {
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::OptiFine => "optifine",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "forge" => LoaderKind::Forge,
            "neoforge" => LoaderKind::NeoForge,
            "fabric" => LoaderKind::Fabric,
            "quilt" => LoaderKind::Quilt,
            "optifine" => LoaderKind::OptiFine,
            _ => return None,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub kind: LoaderKind,
    /// 安装时使用的标识（Forge 为 maven 版本，如 1.20.1-47.2.0；OptiFine 为 type/patch）
    pub id: String,
    /// 展示用版本号
    pub version: String,
    pub mc_version: String,
    pub stable: bool,
    pub recommended: bool,
    pub latest: bool,
    /// 附加说明（例如 OptiFine 兼容的 Forge 版本）
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoaderSpec {
    pub kind: LoaderKind,
    /// LoaderVersion.id
    pub version: String,
}

// ───────────────────────── 版本列表 ─────────────────────────

/// 宽松的版本号比较：按 . - _ + 分段，数字段按数值比较
pub fn cmp_version(a: &str, b: &str) -> Ordering {
    let split = |s: &str| -> Vec<String> {
        s.split(|c: char| c == '.' || c == '-' || c == '_' || c == '+')
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect()
    };
    let (pa, pb) = (split(a), split(b));
    for i in 0..pa.len().max(pb.len()) {
        match (pa.get(i), pb.get(i)) {
            (Some(x), Some(y)) => {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    // 数字段比字母段（beta / pre）更新
                    (Ok(_), Err(_)) => Ordering::Greater,
                    (Err(_), Ok(_)) => Ordering::Less,
                    _ => x.cmp(y),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
            // 1.0 与 1.0-beta：没有后缀的更新
            (Some(x), None) => return if x.parse::<u64>().is_ok() { Ordering::Greater } else { Ordering::Less },
            (None, Some(y)) => return if y.parse::<u64>().is_ok() { Ordering::Less } else { Ordering::Greater },
            (None, None) => break,
        }
    }
    Ordering::Equal
}

/// 列出某个 Minecraft 版本可用的加载器版本（新到旧），并标记推荐版与最新版。
/// 元数据带 1 小时缓存（见 meta.rs）；`force` 为 true 时跳过缓存重新请求。
pub async fn list_versions(core: &Core, kind: LoaderKind, mc_version: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let mc = mc_version.trim();
    if mc.is_empty() {
        bail!("缺少游戏版本");
    }
    let mut list = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric::list(core, kind, mc, force).await?,
        LoaderKind::Forge => forge::list_forge(core, mc, force).await?,
        LoaderKind::NeoForge => forge::list_neoforge(core, mc, force).await?,
        LoaderKind::OptiFine => optifine::list(core, mc, force).await?,
    };
    if list.is_empty() {
        bail!("{} 暂不支持 Minecraft {mc}", kind.display_name());
    }
    if !list.iter().any(|v| v.latest) {
        list[0].latest = true;
    }
    if !list.iter().any(|v| v.recommended) {
        if let Some(v) = list.iter_mut().find(|v| v.stable) {
            v.recommended = true;
        }
    }
    Ok(list)
}

/// 在版本列表里按展示版本或安装标识查找（整合包清单里只有加载器版本号）
pub async fn find_version(core: &Core, kind: LoaderKind, mc: &str, wanted: &str) -> anyhow::Result<LoaderVersion> {
    let list = list_versions(core, kind, mc, false).await?;
    let w = wanted.trim();
    list.iter()
        .find(|v| v.id == w || v.version == w)
        .or_else(|| list.iter().find(|v| v.id.ends_with(&format!("-{w}")) || v.id.contains(&format!("-{w}-"))))
        .cloned()
        .ok_or_else(|| anyhow!("找不到 {} {w}（Minecraft {mc}）", kind.display_name()))
}

/// 展示用版本号
pub fn display_version(kind: LoaderKind, mc: &str, id: &str) -> String {
    match kind {
        LoaderKind::Forge => {
            let v = id.strip_prefix(&format!("{mc}-")).unwrap_or(id);
            v.strip_suffix(&format!("-{mc}")).unwrap_or(v).to_string()
        }
        LoaderKind::NeoForge => id.strip_prefix("1.20.1-").unwrap_or(id).to_string(),
        LoaderKind::OptiFine => id.replace('/', "_"),
        _ => id.to_string(),
    }
}

// ───────────────────────── 组合校验 ─────────────────────────

/// 校验加载器组合，返回 (主加载器, OptiFine)
pub fn validate(specs: &[LoaderSpec]) -> anyhow::Result<(Option<LoaderSpec>, Option<LoaderSpec>)> {
    let mut seen = HashSet::new();
    let mut primary: Option<LoaderSpec> = None;
    let mut optifine: Option<LoaderSpec> = None;
    for s in specs {
        if s.version.trim().is_empty() {
            bail!("请选择 {} 的版本", s.kind.display_name());
        }
        if !seen.insert(s.kind) {
            bail!("{} 重复", s.kind.display_name());
        }
        match s.kind {
            LoaderKind::OptiFine => optifine = Some(s.clone()),
            _ => {
                if let Some(p) = &primary {
                    bail!(
                        "{} 与 {} 不能同时安装（同一实例只能有一个模组加载器）",
                        p.kind.display_name(),
                        s.kind.display_name()
                    );
                }
                primary = Some(s.clone());
            }
        }
    }
    if let (Some(p), Some(_)) = (&primary, &optifine) {
        match p.kind {
            LoaderKind::Forge => {}
            LoaderKind::Fabric | LoaderKind::Quilt => bail!(
                "OptiFine 不能与 {} 一起安装。如需在 {} 上使用 OptiFine，请改装 OptiFabric 模组（或使用 Sodium / Iris 替代）",
                p.kind.display_name(),
                p.kind.display_name()
            ),
            _ => bail!("OptiFine 官方只支持 Forge，不能与 {} 一起安装", p.kind.display_name()),
        }
    }
    Ok((primary, optifine))
}

/// 安装这些加载器需要的任务阶段数（与 install_loaders 中的阶段一一对应）
pub fn stage_count(specs: &[LoaderSpec]) -> u32 {
    let has_primary = specs.iter().any(|s| s.kind != LoaderKind::OptiFine);
    3 + specs
        .iter()
        .map(|s| match s.kind {
            LoaderKind::Fabric | LoaderKind::Quilt => 1,
            LoaderKind::Forge | LoaderKind::NeoForge => 4,
            LoaderKind::OptiFine => {
                if has_primary {
                    1
                } else {
                    3
                }
            }
        })
        .sum::<u32>()
}

/// 随加载器安装的前置模组（Fabric API / QFAPI）占用的阶段数
pub fn addon_stages(addons: &[InstallItem]) -> u32 {
    if addons.is_empty() {
        0
    } else {
        1
    }
}

/// 阶段计数器：多个安装步骤串在同一个任务里时，保证“阶段 x/y”连续
pub struct Stager {
    task: Arc<TaskHandle>,
    pub index: u32,
    pub total: u32,
}

impl Stager {
    /// `done` 为已经完成的阶段数
    pub fn new(task: Arc<TaskHandle>, done: u32, total: u32) -> Self {
        Self { task, index: done, total: total.max(done + 1) }
    }

    /// 进入下一阶段并显示标签
    pub fn next(&mut self, label: impl Into<String>, files: u64, bytes: u64) -> (u32, u32) {
        self.index = (self.index + 1).min(self.total);
        self.task.set_stage(self.index, self.total, label, files, bytes);
        (self.index, self.total)
    }

    /// 进入下一阶段（由被调用的函数自己设置标签）
    pub fn take(&mut self) -> (u32, u32) {
        self.index = (self.index + 1).min(self.total);
        (self.index, self.total)
    }

    pub fn skip(&mut self, n: u32) {
        self.index = (self.index + n).min(self.total);
    }
}

// ───────────────────────── JSON 合并 ─────────────────────────

/// 把子版本（加载器 profile）合并进父版本：
/// libraries 子在前（同名库去重）、arguments.game / jvm 父在前子在后、其余字段子版本覆盖；
/// 去掉 inheritsFrom 与 jar，使结果成为自包含的实例 JSON（客户端 jar 即实例目录下的 <实例名>.jar）。
pub fn merge_json(parent: &Value, child: &Value) -> Value {
    let mut out = parent.as_object().cloned().unwrap_or_default();
    let Some(c) = child.as_object() else {
        return Value::Object(out);
    };
    for (k, v) in c {
        match k.as_str() {
            "id" | "inheritsFrom" | "jar" => {}
            "libraries" => {
                let mut libs: Vec<Value> = v.as_array().cloned().unwrap_or_default();
                let names: HashSet<String> = libs
                    .iter()
                    .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect();
                if let Some(p) = out.get("libraries").and_then(|x| x.as_array()) {
                    libs.extend(
                        p.iter()
                            .filter(|l| {
                                l.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|n| !names.contains(n))
                                    .unwrap_or(true)
                            })
                            .cloned(),
                    );
                }
                out.insert("libraries".into(), Value::Array(libs));
            }
            "arguments" => {
                let mut merged = match out.get("arguments") {
                    Some(Value::Object(o)) => Value::Object(o.clone()),
                    _ => json!({}),
                };
                for key in ["game", "jvm"] {
                    if let Some(extra) = v.get(key).and_then(|x| x.as_array()) {
                        let mut arr = merged.get(key).and_then(|x| x.as_array()).cloned().unwrap_or_default();
                        arr.extend(extra.iter().cloned());
                        merged[key] = Value::Array(arr);
                    }
                }
                out.insert("arguments".into(), merged);
            }
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    out.remove("inheritsFrom");
    out.remove("jar");
    Value::Object(out)
}

fn looks_vanilla(v: &Value) -> bool {
    let main = v.get("mainClass").and_then(|m| m.as_str()).unwrap_or("");
    let legacy_args = v.get("minecraftArguments").and_then(|m| m.as_str()).unwrap_or("");
    if v.get("inheritsFrom").is_some() {
        return false;
    }
    match main {
        "net.minecraft.client.main.Main" => true,
        // 远古版本自带 launchwrapper，但没有 Forge / OptiFine 的 tweaker
        "net.minecraft.launchwrapper.Launch" => {
            !legacy_args.contains("FMLTweaker") && !legacy_args.contains("OptiFineTweaker")
        }
        _ => false,
    }
}

/// 取得实例的原版 JSON（并写入备份）
async fn vanilla_base(core: &Core, name: &str, mc: &str) -> anyhow::Result<Value> {
    let dirs = core.game_dirs();
    let backup = dirs.instance_dir(name).join(VANILLA_BACKUP);
    if let Ok(v) = read_json::<Value>(&backup) {
        if v.get("mainClass").is_some() {
            return Ok(v);
        }
    }
    let meta = instance::load_meta(&dirs, name)?;
    let current: Option<Value> = read_json(&dirs.version_json(name)).ok();
    let mut v = match current {
        Some(c) if meta.loader.is_none() && meta.optifine.is_none() && looks_vanilla(&c) => c,
        _ => install::fetch_version_json(core, mc).await?,
    };
    v["id"] = Value::String(name.to_string());
    v["clientVersion"] = Value::String(mc.to_string());
    write_json_atomic(&backup, &v)?;
    Ok(v)
}

// ───────────────────────── Java / 进程 ─────────────────────────

/// 实例对应的 Java：实例覆盖 → 全局覆盖 → 官方运行时（缺失时自动下载）
pub async fn instance_java(
    core: &Core,
    task: &Arc<TaskHandle>,
    name: &str,
    stage: (u32, u32),
) -> anyhow::Result<JavaInfo> {
    let dirs = core.game_dirs();
    let meta = instance::load_meta(&dirs, name)?;
    let settings = core.settings();
    if let Some(p) = meta.java_path.clone().or(settings.java_path.clone()) {
        return detect::inspect(&p, "自定义").ok_or_else(|| anyhow!("找不到指定的 Java：{p}"));
    }
    let version = load_resolved(&dirs, name)?;
    let (component, major) = version.java_requirement();
    runtime::ensure_runtime(core, task, component.as_deref(), major, stage).await
}

/// 运行安装类 Java 程序时优先用 java.exe（有控制台输出），没有再用 javaw.exe
pub fn console_java(info: &JavaInfo) -> PathBuf {
    let p = PathBuf::from(&info.path);
    if p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("javaw.exe")).unwrap_or(false) {
        let j = p.with_file_name("java.exe");
        if j.is_file() {
            return j;
        }
    }
    p
}

/// 运行 Java 进程直到结束；失败时附带输出末尾若干行。任务取消时结束进程。
pub async fn run_java(java: &Path, args: &[String], cwd: &Path, task: &TaskHandle) -> anyhow::Result<()> {
    std::fs::create_dir_all(cwd)?;
    let mut cmd = tokio::process::Command::new(java);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().with_context(|| format!("无法启动 Java：{}", java.display()))?;
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let out_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut buf).await;
        }
        buf
    });
    let err_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_end(&mut buf).await;
        }
        buf
    });
    let status = loop {
        tokio::select! {
            s = child.wait() => break s?,
            _ = tokio::time::sleep(Duration::from_millis(300)) => {
                if task.is_cancelled() {
                    let _ = child.kill().await;
                    bail!("任务已取消");
                }
            }
        }
    };
    let mut text = String::from_utf8_lossy(&out_task.await.unwrap_or_default()).to_string();
    text.push_str(&String::from_utf8_lossy(&err_task.await.unwrap_or_default()));
    if !status.success() {
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let tail = lines[lines.len().saturating_sub(25)..].join("\n");
        bail!("Java 进程退出码 {:?}：\n{}", status.code(), tail);
    }
    Ok(())
}

pub fn download_options(core: &Core, verify: Verify) -> DownloadOptions {
    let s = core.settings();
    DownloadOptions { concurrency: s.concurrency, retries: s.retries, verify }
}

// ───────────────────────── 安装入口 ─────────────────────────

/// 为已有实例安装 / 更换加载器（在后台任务中运行），返回任务 id。
/// `addons` 为加载器装好后要下载到 mods 的前置模组（Fabric API / QFAPI），可以为空。
pub fn start_install(core: &Arc<Core>, name: String, specs: Vec<LoaderSpec>, addons: Vec<InstallItem>) -> anyhow::Result<String> {
    validate(&specs)?;
    if specs.is_empty() {
        bail!("请至少选择一个加载器");
    }
    validate_addons(&specs, &addons)?;
    let dirs = core.game_dirs();
    if !dirs.version_json(&name).is_file() {
        bail!("实例 {name} 缺少版本文件，请先修复或重新安装");
    }
    if core.is_running(&name) {
        bail!("实例正在运行，请先关闭游戏");
    }
    let meta = instance::load_meta(&dirs, &name)?;
    if !instance::meta_path(&dirs, &name).is_file() {
        let v: Value = read_json(&dirs.version_json(&name))?;
        if v.get("inheritsFrom").is_some() || !looks_vanilla(&v) {
            bail!("这是从其他启动器复制来的、已带加载器的外部版本，不支持补装。请在 PilotLauncher 中新建实例");
        }
    }
    let mut label = specs
        .iter()
        .map(|s| format!("{} {}", s.kind.display_name(), display_version(s.kind, &meta.mc_version, &s.version)))
        .collect::<Vec<_>>()
        .join(" + ");
    for a in &addons {
        label.push_str(&format!(" + {}", a.title));
    }
    let task = core.tasks.create(format!("为 {name} 安装 {label}"), "install");
    let id = task.id.clone();
    let core2 = core.clone();
    crate::tasks::spawn_task(task, move |task| async move {
        let total = stage_count(&specs) + addon_stages(&addons);
        let mut st = Stager::new(task.clone(), 0, total);
        install_loaders(&core2, &task, &name, &specs, &mut st).await?;
        install_addons(&core2, &task, &name, addons, &mut st).await
    });
    Ok(id)
}

/// 前置模组只能随 Fabric / Quilt 安装，且必须来自 Modrinth
pub fn validate_addons(specs: &[LoaderSpec], addons: &[InstallItem]) -> anyhow::Result<()> {
    if addons.is_empty() {
        return Ok(());
    }
    let primary = specs.iter().find(|s| s.kind != LoaderKind::OptiFine).map(|s| s.kind);
    if !matches!(primary, Some(LoaderKind::Fabric) | Some(LoaderKind::Quilt)) {
        bail!("Fabric API / QFAPI 只能与 Fabric / Quilt 一起安装");
    }
    if addons.len() > 4 {
        bail!("前置模组太多");
    }
    Ok(())
}

/// 加载器装好后，把前置模组（Fabric API / QFAPI）下载到实例的 mods 文件夹。
/// 复用社区资源的安装代码：遵守社区资源源策略（MCIM 镜像）、校验哈希、写入 glass-resources.json。
/// 失败时任务显示为失败，但加载器本身已经装好、实例可以正常启动。
pub async fn install_addons(
    core: &Arc<Core>,
    task: &Arc<TaskHandle>,
    name: &str,
    addons: Vec<InstallItem>,
    st: &mut Stager,
) -> anyhow::Result<()> {
    if addons.is_empty() {
        return Ok(());
    }
    let titles = addons.iter().map(|a| a.title.clone()).collect::<Vec<_>>().join("、");
    let stage = st.take();
    crate::community::install_addons(core, task, name, addons, stage)
        .await
        .with_context(|| format!("{titles} 下载失败（加载器已经安装完成，可以稍后在“下载 → 模组”中手动安装）"))
}

/// 在当前任务中安装加载器。实例的原版文件应已存在（或可下载）。
pub async fn install_loaders(
    core: &Arc<Core>,
    task: &Arc<TaskHandle>,
    name: &str,
    specs: &[LoaderSpec],
    st: &mut Stager,
) -> anyhow::Result<()> {
    let (primary, optifine) = validate(specs)?;
    let dirs = core.game_dirs();
    let mut meta = instance::load_meta(&dirs, name)?;
    let mc = meta.mc_version.clone();

    // 1. 恢复原版 JSON（失败时实例仍是可用的原版）
    st.next("准备原版版本文件", 0, 0);
    let mut json = vanilla_base(core, name, &mc).await?;
    write_json_atomic(&dirs.version_json(name), &json)?;
    meta.loader = None;
    meta.optifine = None;
    instance::save_meta(&dirs, &meta)?;

    // 2. processors 需要客户端 jar 与原版库
    let stage = st.take();
    install::complete_files(core, task, name, Verify::Quick, stage).await?;

    // 3. 主加载器
    if let Some(p) = &primary {
        match p.kind {
            LoaderKind::Fabric | LoaderKind::Quilt => {
                fabric::install(core, task, st, p.kind, &mc, &p.version, &mut json).await?
            }
            LoaderKind::Forge | LoaderKind::NeoForge => {
                forge::install(core, task, st, p.kind, name, &mc, &p.version, &mut json).await?
            }
            LoaderKind::OptiFine => unreachable!(),
        }
    }
    // 4. OptiFine：与 Forge 共存时放进 mods，单独安装时生成 launchwrapper 版本
    if let Some(o) = &optifine {
        optifine::install(core, task, st, name, &mc, &o.version, primary.is_some(), &mut json).await?;
    }

    json["id"] = Value::String(name.to_string());
    json["clientVersion"] = Value::String(mc.clone());
    write_json_atomic(&dirs.version_json(name), &json)?;

    // 5. 补全加载器依赖库
    let stage = st.take();
    install::complete_files(core, task, name, Verify::Quick, stage).await?;

    meta = instance::load_meta(&dirs, name)?;
    meta.loader = match (&primary, &optifine) {
        (Some(p), _) => Some(LoaderInfo { kind: p.kind.id().into(), version: display_version(p.kind, &mc, &p.version) }),
        (None, Some(o)) => Some(LoaderInfo {
            kind: LoaderKind::OptiFine.id().into(),
            version: display_version(LoaderKind::OptiFine, &mc, &o.version),
        }),
        _ => None,
    };
    meta.optifine = optifine.as_ref().map(|o| display_version(LoaderKind::OptiFine, &mc, &o.version));
    meta.installed = true;
    instance::save_meta(&dirs, &meta)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert_eq!(cmp_version("47.2.0", "47.10.0"), Ordering::Less);
        assert_eq!(cmp_version("21.1.77", "21.1.8"), Ordering::Greater);
        assert_eq!(cmp_version("20.4.80-beta", "20.4.80"), Ordering::Less);
        assert_eq!(cmp_version("26.1.0.10-beta", "26.1.0.9-beta"), Ordering::Greater);
    }

    #[test]
    fn merge_child_first() {
        let parent = json!({"id":"x","mainClass":"A","libraries":[{"name":"a:b:1"}],
            "arguments":{"game":["--a"],"jvm":["-cp","${classpath}"]}});
        let child = json!({"id":"fabric","inheritsFrom":"1.20.1","mainClass":"B",
            "libraries":[{"name":"c:d:1"},{"name":"a:b:1"}],"arguments":{"game":["--b"],"jvm":["-Dx"]}});
        let m = merge_json(&parent, &child);
        assert_eq!(m["mainClass"], "B");
        assert_eq!(m["libraries"].as_array().unwrap().len(), 2);
        assert_eq!(m["arguments"]["game"], json!(["--a", "--b"]));
        assert!(m.get("inheritsFrom").is_none());
    }

    #[test]
    fn combos() {
        let s = |k| LoaderSpec { kind: k, version: "1".into() };
        assert!(validate(&[s(LoaderKind::Forge), s(LoaderKind::OptiFine)]).is_ok());
        assert!(validate(&[s(LoaderKind::Fabric), s(LoaderKind::OptiFine)]).is_err());
        assert!(validate(&[s(LoaderKind::Fabric), s(LoaderKind::Forge)]).is_err());
        assert!(validate(&[s(LoaderKind::OptiFine)]).is_ok());
    }
}
