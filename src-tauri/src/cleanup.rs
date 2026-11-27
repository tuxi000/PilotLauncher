//! 清理游戏垃圾：启动器缓存、游戏日志、崩溃报告、不再使用的旧组件。
//!
//! 安全原则：
//! - 只在下面列出的明确位置里找文件，绝不碰存档、mods、config、资源包、光影包、截图；
//! - 扫描和清理都由后端重新计算目标，前端只传类别 id，不传路径；
//! - 不跟随符号链接和目录联接；
//! - 单个文件删除失败（被占用、没有权限）只计数，不中断；
//! - 正在运行的实例跳过它的日志；有下载 / 安装任务进行中时拒绝清理（不能动下载中的 .part 和安装器临时目录）；
//! - 旧组件判断不了是否还在用的，一律保留。

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use anyhow::bail;
use serde::Serialize;
use serde_json::Value;

use crate::auth::skin;
use crate::instance;
use crate::java::runtime;
use crate::minecraft::version::load_resolved;
use crate::paths::{GameDirs, Paths};
use crate::state::Core;
use crate::terracotta::metadata::{PACKAGE_NAME as TERRACOTTA_PACKAGE, VERSION as TERRACOTTA_VERSION};
use crate::util::read_json;

const CAT_CACHE: &str = "cache";
const CAT_LOGS: &str = "logs";
const CAT_CRASH: &str = "crash";
const CAT_COMPONENTS: &str = "components";

/// 遍历时最多访问的条目数（防止在异常巨大的目录里卡住）
const WALK_LIMIT: usize = 400_000;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupDetail {
    pub label: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCategory {
    pub id: String,
    pub label: String,
    pub description: String,
    pub files: u64,
    pub bytes: u64,
    pub default_selected: bool,
    /// 这一类里具体有哪些（例如“加载器版本列表缓存”“1.20.1 生存”）
    pub details: Vec<CleanupDetail>,
    /// 跳过了什么、为什么（例如正在运行的实例）
    pub notes: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupScan {
    pub categories: Vec<CleanupCategory>,
    pub total_bytes: u64,
    /// 有下载 / 安装任务进行中：这时不能清理
    pub busy: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanupOutcome {
    pub id: String,
    /// 实际删除的文件数与字节数
    pub files: u64,
    pub bytes: u64,
    /// 删除失败（被占用等）的文件数
    pub failed: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub categories: Vec<CleanupOutcome>,
    pub freed_bytes: u64,
    pub deleted_files: u64,
    pub failed_files: u64,
}

/// 清理所需的状态快照（在异步命令里取好，再交给阻塞线程）
pub struct CleanCtx {
    paths: Paths,
    dirs: GameDirs,
    /// 正在运行或正在启动的实例
    running: HashSet<String>,
    global_java: Option<String>,
    any_game: bool,
    busy: bool,
    terracotta_installing: bool,
}

impl CleanCtx {
    pub fn from_core(core: &Core) -> Self {
        let mut running: HashSet<String> = core.running_instances().into_iter().collect();
        {
            let launching = core.launching.lock().unwrap_or_else(|e| e.into_inner());
            running.extend(launching.iter().cloned());
        }
        let any_game = !running.is_empty();
        let busy = core.tasks.list().iter().any(|t| t.status == "running");
        Self {
            paths: core.paths.clone(),
            dirs: core.game_dirs(),
            running,
            global_java: core.settings().java_path,
            any_game,
            busy,
            terracotta_installing: core.terracotta.installing.load(Ordering::SeqCst),
        }
    }
}

enum Target {
    File {
        path: PathBuf,
        bytes: u64,
    },
    /// 整个目录：递归删除其中的文件。keep_root 为 true 时保留目录本身；
    /// first 是要最先删除的文件（Java 运行时的标记文件，删掉它之后即使后面有文件删不掉，运行时也不会被误认为完整）
    Dir {
        path: PathBuf,
        files: u64,
        bytes: u64,
        keep_root: bool,
        first: Option<PathBuf>,
    },
}

impl Target {
    fn files(&self) -> u64 {
        match self {
            Target::File { .. } => 1,
            Target::Dir { files, .. } => *files,
        }
    }
    fn bytes(&self) -> u64 {
        match self {
            Target::File { bytes, .. } => *bytes,
            Target::Dir { bytes, .. } => *bytes,
        }
    }
}

#[derive(Default)]
struct Plan {
    targets: Vec<Target>,
    details: Vec<CleanupDetail>,
    notes: Vec<String>,
}

impl Plan {
    fn files(&self) -> u64 {
        self.targets.iter().map(|t| t.files()).sum()
    }
    fn bytes(&self) -> u64 {
        self.targets.iter().map(|t| t.bytes()).sum()
    }
}

// ───────────────────────── 遍历工具 ─────────────────────────

/// 递归统计目录里的文件数与字节数（不跟随链接）
fn dir_usage(dir: &Path) -> (u64, u64) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut seen = 0usize;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            seen += 1;
            if seen > WALK_LIMIT {
                return (files, bytes);
            }
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    (files, bytes)
}

fn is_leftover(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".part") || n.ends_with(".glass-tmp")
}

/// 在 root 下查找下载中断留下的临时文件（*.part / *.glass-tmp），最多深入 max_depth 层
fn find_leftovers(root: &Path, max_depth: usize, out: &mut Vec<(PathBuf, u64)>) {
    let mut seen = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            seen += 1;
            if seen > WALK_LIMIT {
                return;
            }
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                if depth < max_depth {
                    stack.push((entry.path(), depth + 1));
                }
            } else if ft.is_file() && is_leftover(&entry.file_name().to_string_lossy()) {
                let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.push((entry.path(), len));
            }
        }
    }
}

/// Forge / NeoForge 安装器：装好之后不再需要（启动时不会用到，重装时会重新下载）
fn find_installers(libraries: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let roots = [
        libraries.join("net").join("minecraftforge").join("forge"),
        libraries.join("net").join("neoforged").join("neoforge"),
        libraries.join("net").join("neoforged").join("forge"),
    ];
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else {
            continue;
        };
        for ver in rd.flatten() {
            let Ok(ft) = ver.file_type() else {
                continue;
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(ver.path()) else {
                continue;
            };
            for f in files.flatten() {
                let name = f.file_name().to_string_lossy().to_ascii_lowercase();
                let is_file = f.file_type().map(|t| t.is_file()).unwrap_or(false);
                if is_file && name.ends_with("-installer.jar") {
                    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
                    out.push((f.path(), len));
                }
            }
        }
    }
}

/// versions/ 下的所有文件夹（实例目录，包括其他启动器的版本）
fn instance_dirs(dirs: &GameDirs) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dirs.versions()) {
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            out.push((entry.file_name().to_string_lossy().to_string(), entry.path()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ───────────────────────── 各类别 ─────────────────────────

fn cache_label(name: &str) -> String {
    match name {
        "version_manifest_v2.json" => "游戏版本列表缓存".into(),
        "loader-meta" => "加载器版本列表缓存".into(),
        "downloads" => "地图下载残留".into(),
        "modpacks" => "整合包下载残留".into(),
        "optifine" => "OptiFine 安装器".into(),
        "installer-tmp" => "安装器临时文件".into(),
        n if n == TERRACOTTA_PACKAGE => "联机组件安装包（已解压）".into(),
        n => n.to_string(),
    }
}

/// 1. 启动器缓存：cache/ 下除皮肤缓存以外的所有内容 + 各处下载中断留下的 .part + 用过的 Forge 安装器
fn plan_cache(c: &CleanCtx) -> Plan {
    let mut plan = Plan::default();
    let cache = c.paths.cache_dir();
    if let Ok(rd) = std::fs::read_dir(&cache) {
        let mut entries: Vec<std::fs::DirEntry> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            // 皮肤缓存很小，删掉后头像要重新下载，保留
            if name.eq_ignore_ascii_case(skin::CACHE_DIR) {
                continue;
            }
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            let path = entry.path();
            let label = cache_label(&name);
            if ft.is_dir() {
                let (files, bytes) = dir_usage(&path);
                if files == 0 {
                    continue;
                }
                plan.targets.push(Target::Dir {
                    path,
                    files,
                    bytes,
                    keep_root: true,
                    first: None,
                });
                plan.details.push(CleanupDetail { label, files, bytes });
            } else if ft.is_file() {
                let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                plan.targets.push(Target::File { path, bytes });
                plan.details.push(CleanupDetail { label, files: 1, bytes });
            }
        }
    }

    let mut leftovers: Vec<(PathBuf, u64)> = Vec::new();
    find_leftovers(&c.paths.runtime_dir(), 12, &mut leftovers);
    find_leftovers(&c.paths.authlib_dir(), 1, &mut leftovers);
    find_leftovers(&c.dirs.libraries(), 16, &mut leftovers);
    find_leftovers(&c.dirs.assets(), 4, &mut leftovers);
    for (name, dir) in instance_dirs(&c.dirs) {
        if c.running.contains(&name) {
            continue;
        }
        find_leftovers(&dir, 0, &mut leftovers);
        for sub in ["mods", "resourcepacks", "shaderpacks"] {
            find_leftovers(&dir.join(sub), 0, &mut leftovers);
        }
    }
    if !leftovers.is_empty() {
        let bytes: u64 = leftovers.iter().map(|(_, b)| *b).sum();
        plan.details.push(CleanupDetail {
            label: "下载中断留下的临时文件（.part）".into(),
            files: leftovers.len() as u64,
            bytes,
        });
        for (path, bytes) in leftovers {
            plan.targets.push(Target::File { path, bytes });
        }
    }

    let mut installers: Vec<(PathBuf, u64)> = Vec::new();
    find_installers(&c.dirs.libraries(), &mut installers);
    if !installers.is_empty() {
        let bytes: u64 = installers.iter().map(|(_, b)| *b).sum();
        plan.details.push(CleanupDetail {
            label: "Forge / NeoForge 安装器（已安装完成，不再需要）".into(),
            files: installers.len() as u64,
            bytes,
        });
        for (path, bytes) in installers {
            plan.targets.push(Target::File { path, bytes });
        }
    }
    plan
}

/// 2. 游戏日志：versions/<实例>/logs/。正在运行的实例跳过
fn plan_logs(c: &CleanCtx) -> Plan {
    let mut plan = Plan::default();
    for (name, dir) in instance_dirs(&c.dirs) {
        let logs = dir.join("logs");
        if !logs.is_dir() {
            continue;
        }
        let (files, bytes) = dir_usage(&logs);
        if files == 0 {
            continue;
        }
        if c.running.contains(&name) {
            plan.notes.push(format!("{name} 正在运行，跳过它的日志"));
            continue;
        }
        plan.targets.push(Target::Dir {
            path: logs,
            files,
            bytes,
            keep_root: true,
            first: None,
        });
        plan.details.push(CleanupDetail { label: name, files, bytes });
    }
    // 启动器自己不写日志文件（游戏日志在内存里，日志窗口关闭即释放），这里没有别的要清
    plan
}

/// 3. 崩溃报告：versions/<实例>/crash-reports/ 和实例目录下的 hs_err_pid*.log / replay_pid*.log
fn plan_crash(c: &CleanCtx) -> Plan {
    let mut plan = Plan::default();
    for (name, dir) in instance_dirs(&c.dirs) {
        let mut files = 0u64;
        let mut bytes = 0u64;
        let reports = dir.join("crash-reports");
        if reports.is_dir() {
            let (f, b) = dir_usage(&reports);
            if f > 0 {
                plan.targets.push(Target::Dir {
                    path: reports,
                    files: f,
                    bytes: b,
                    keep_root: true,
                    first: None,
                });
                files += f;
                bytes += b;
            }
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let n = entry.file_name().to_string_lossy().to_ascii_lowercase();
                let jvm_crash = (n.starts_with("hs_err_pid") || n.starts_with("replay_pid")) && n.ends_with(".log");
                if !jvm_crash {
                    continue;
                }
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                let b = entry.metadata().map(|m| m.len()).unwrap_or(0);
                plan.targets.push(Target::File { path: entry.path(), bytes: b });
                files += 1;
                bytes += b;
            }
        }
        if files > 0 {
            plan.details.push(CleanupDetail { label: name, files, bytes });
        }
    }
    plan
}

/// 实例需要的 Java：组件名（与 runtime/ 下的目录名一致）和“只按主版本找”的主版本
#[derive(Default)]
struct JavaNeed {
    components: HashSet<String>,
    majors: HashSet<u32>,
}

impl JavaNeed {
    fn add(&mut self, runtime_root: &Path, component: Option<String>, major: u32) {
        match component {
            Some(comp) => {
                // 组件目录不存在时，启动器会按主版本兜底（可能装在别的组件名下），这个主版本的运行时都保留
                if !runtime_root.join(&comp).join(runtime::MARKER).is_file() {
                    self.majors.insert(major);
                }
                self.components.insert(comp.to_ascii_lowercase());
            }
            None => {
                self.majors.insert(major);
            }
        }
    }

    fn keeps(&self, component: &str, major: u32) -> bool {
        self.components.contains(&component.to_ascii_lowercase()) || self.majors.contains(&major)
    }
}

fn java_of(j: &Value) -> (Option<String>, u32) {
    let comp = j
        .get("component")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let major = j.get("majorVersion").and_then(|x| x.as_u64()).unwrap_or(8) as u32;
    (comp, major)
}

/// 版本链读不全时，直接看实例 JSON（以及父版本 JSON）里的 javaVersion
fn raw_java_requirement(dirs: &GameDirs, name: &str) -> Option<(Option<String>, u32)> {
    let v: Value = read_json(&dirs.version_json(name)).ok()?;
    if let Some(j) = v.get("javaVersion") {
        return Some(java_of(j));
    }
    match v.get("inheritsFrom").and_then(|x| x.as_str()) {
        Some(parent) => {
            let p: Value = read_json(&dirs.version_json(parent)).ok()?;
            p.get("javaVersion").map(java_of)
        }
        // 与 VersionJson::java_requirement 的缺省值一致
        None => Some((Some("jre-legacy".to_string()), 8)),
    }
}

/// 手动指定的 Java 如果在 runtime/<组件>/ 里，返回组件名
fn runtime_component_of(runtime_root: &Path, java: &str) -> Option<String> {
    let root = runtime_root.display().to_string().replace('/', "\\").to_ascii_lowercase();
    let path = java.trim().trim_matches('"').replace('/', "\\").to_ascii_lowercase();
    let rest = path.strip_prefix(root.as_str())?.trim_start_matches('\\');
    let comp = rest.split('\\').next()?;
    if comp.is_empty() {
        None
    } else {
        Some(comp.to_string())
    }
}

fn needed_java(c: &CleanCtx) -> Result<JavaNeed, String> {
    let runtime_root = c.paths.runtime_dir();
    let mut need = JavaNeed::default();
    let mut overrides: Vec<String> = c.global_java.iter().cloned().collect();
    for (name, _) in instance_dirs(&c.dirs) {
        let has_meta = instance::meta_path(&c.dirs, &name).is_file();
        let has_json = c.dirs.version_json(&name).is_file();
        if !has_meta && !has_json {
            continue;
        }
        match instance::load_meta(&c.dirs, &name) {
            Ok(meta) => {
                if let Some(p) = meta.java_path {
                    overrides.push(p);
                }
            }
            Err(_) => return Err(format!("读不出实例 {name} 的设置，为安全起见本次不清理 Java 运行时")),
        }
        // 还没装完的实例（没有版本 JSON）现在启动不了，修复时会按需重新下载 Java
        if !has_json {
            continue;
        }
        match load_resolved(&c.dirs, &name) {
            Ok(v) => {
                let (comp, major) = v.java_requirement();
                need.add(&runtime_root, comp, major);
            }
            Err(_) => match raw_java_requirement(&c.dirs, &name) {
                Some((comp, major)) => need.add(&runtime_root, comp, major),
                None => {
                    return Err(format!(
                        "实例 {name} 的版本文件不完整，判断不了它需要哪个 Java，本次不清理 Java 运行时"
                    ))
                }
            },
        }
    }
    for p in overrides {
        if let Some(comp) = runtime_component_of(&runtime_root, &p) {
            need.components.insert(comp);
        }
    }
    Ok(need)
}

/// 4. 不再使用的旧组件：没有实例需要的 Java 运行时、旧版本的联机组件
fn plan_components(c: &CleanCtx) -> Plan {
    let mut plan = Plan::default();

    // Java 运行时
    if c.any_game {
        plan.notes.push("有游戏正在运行，本次不清理 Java 运行时".into());
    } else {
        match needed_java(c) {
            Err(why) => plan.notes.push(why),
            Ok(need) => {
                let root = c.paths.runtime_dir();
                if let Ok(rd) = std::fs::read_dir(&root) {
                    let mut entries: Vec<std::fs::DirEntry> = rd.flatten().collect();
                    entries.sort_by_key(|e| e.file_name());
                    for entry in entries {
                        let Ok(ft) = entry.file_type() else {
                            continue;
                        };
                        if !ft.is_dir() || ft.is_symlink() {
                            continue;
                        }
                        let comp = entry.file_name().to_string_lossy().to_string();
                        let dir = entry.path();
                        let marker_path = dir.join(runtime::MARKER);
                        // 没有标记文件：可能是下载到一半的目录或别的程序放的，判断不了，不删
                        let Ok(marker) = read_json::<Value>(&marker_path) else {
                            continue;
                        };
                        let major = marker.get("major").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                        if major == 0 || need.keeps(&comp, major) {
                            continue;
                        }
                        let (files, bytes) = dir_usage(&dir);
                        plan.details.push(CleanupDetail {
                            label: format!("Java 运行时 {comp}（Java {major}，没有实例使用）"),
                            files,
                            bytes,
                        });
                        plan.targets.push(Target::Dir {
                            path: dir,
                            files,
                            bytes,
                            keep_root: false,
                            first: Some(marker_path),
                        });
                    }
                }
            }
        }
    }

    // 联机组件：只保留当前版本目录和运行时用的 run/
    if c.terracotta_installing {
        plan.notes.push("联机组件正在安装，本次不清理旧版本".into());
    } else if let Ok(rd) = std::fs::read_dir(c.paths.terracotta_dir()) {
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == TERRACOTTA_VERSION || name == "run" {
                continue;
            }
            let looks_like_version = name.chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(false);
            let temp = name.starts_with(".old-") || name.starts_with(".install-");
            if !looks_like_version && !temp {
                continue;
            }
            let dir = entry.path();
            let (files, bytes) = dir_usage(&dir);
            plan.details.push(CleanupDetail {
                label: if temp {
                    "联机组件安装残留".to_string()
                } else {
                    format!("联机组件旧版本 {name}")
                },
                files,
                bytes,
            });
            plan.targets.push(Target::Dir {
                path: dir,
                files,
                bytes,
                keep_root: false,
                first: None,
            });
        }
    }
    plan
}

fn plan_for(c: &CleanCtx, id: &str) -> Option<Plan> {
    match id {
        CAT_CACHE => Some(plan_cache(c)),
        CAT_LOGS => Some(plan_logs(c)),
        CAT_CRASH => Some(plan_crash(c)),
        CAT_COMPONENTS => Some(plan_components(c)),
        _ => None,
    }
}

// ───────────────────────── 删除 ─────────────────────────

fn delete_file(path: &Path, out: &mut CleanupOutcome) {
    let len = match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => return,
        Ok(m) => m.len(),
        // 已经不在了
        Err(_) => return,
    };
    let mut result = std::fs::remove_file(path);
    if result.is_err() {
        // 只读文件：去掉只读属性再试一次
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perm = meta.permissions();
            if perm.readonly() {
                perm.set_readonly(false);
                if std::fs::set_permissions(path, perm).is_ok() {
                    result = std::fs::remove_file(path);
                }
            }
        }
    }
    match result {
        Ok(()) => {
            out.files += 1;
            out.bytes += len;
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(_) => out.failed += 1,
    }
}

fn delete_tree(dir: &Path, out: &mut CleanupOutcome, depth: usize) {
    if depth > 64 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else {
            out.failed += 1;
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            delete_tree(&path, out, depth + 1);
            let _ = std::fs::remove_dir(&path);
        } else {
            delete_file(&path, out);
        }
    }
}

fn execute(plan: Plan, id: &str) -> CleanupOutcome {
    let mut out = CleanupOutcome {
        id: id.to_string(),
        ..Default::default()
    };
    for target in plan.targets {
        match target {
            Target::File { path, .. } => delete_file(&path, &mut out),
            Target::Dir { path, keep_root, first, .. } => {
                if let Some(f) = first {
                    let before = out.failed;
                    delete_file(&f, &mut out);
                    if out.failed > before {
                        // 标记文件都删不掉（多半正在使用），整个目录保留
                        continue;
                    }
                }
                delete_tree(&path, &mut out, 0);
                if !keep_root {
                    let _ = std::fs::remove_dir(&path);
                }
            }
        }
    }
    out
}

// ───────────────────────── 入口 ─────────────────────────

/// (id, 名称, 说明, 默认勾选)
const CATEGORIES: [(&str, &str, &str, bool); 4] = [
    (
        CAT_CACHE,
        "启动器缓存",
        "版本列表、加载器版本列表、下载中断留下的临时文件、用过的安装包。删除后需要时会重新下载",
        true,
    ),
    (CAT_LOGS, "游戏日志", "各实例 logs 文件夹里的 latest.log、debug.log 和历史日志压缩包", true),
    (CAT_CRASH, "崩溃报告", "各实例的 crash-reports 和 Java 崩溃时留下的 hs_err_pid 日志", true),
    (
        CAT_COMPONENTS,
        "不再使用的旧组件",
        "没有任何实例需要的 Java 运行时、旧版本的联机组件。以后需要时会自动重新下载",
        false,
    ),
];

pub fn scan(c: &CleanCtx) -> CleanupScan {
    let mut categories = Vec::new();
    let mut total_bytes = 0u64;
    for (id, label, description, default_selected) in CATEGORIES {
        let Some(plan) = plan_for(c, id) else {
            continue;
        };
        let files = plan.files();
        let bytes = plan.bytes();
        total_bytes += bytes;
        categories.push(CleanupCategory {
            id: id.to_string(),
            label: label.to_string(),
            description: description.to_string(),
            files,
            bytes,
            default_selected,
            details: plan.details,
            notes: plan.notes,
        });
    }
    CleanupScan {
        categories,
        total_bytes,
        busy: c.busy,
    }
}

pub fn run(c: &CleanCtx, ids: &[String]) -> anyhow::Result<CleanupResult> {
    if c.busy {
        bail!("有下载或安装任务正在进行，请等它们结束后再清理");
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut outcomes: Vec<CleanupOutcome> = Vec::new();
    for id in ids {
        let id = id.as_str();
        if !seen.insert(id) {
            continue;
        }
        let Some(plan) = plan_for(c, id) else {
            continue;
        };
        outcomes.push(execute(plan, id));
    }
    let freed_bytes: u64 = outcomes.iter().map(|o| o.bytes).sum();
    let deleted_files: u64 = outcomes.iter().map(|o| o.files).sum();
    let failed_files: u64 = outcomes.iter().map(|o| o.failed).sum();
    Ok(CleanupResult {
        categories: outcomes,
        freed_bytes,
        deleted_files,
        failed_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftover_names() {
        assert!(is_leftover("client.jar.part"));
        assert!(is_leftover("A.PART"));
        assert!(is_leftover("forge.jar.glass-tmp"));
        assert!(!is_leftover("partition.dat"));
    }

    #[test]
    fn component_of_override() {
        let root = Path::new(r"D:\Games\.minecraft\PilotLauncher\runtime");
        assert_eq!(
            runtime_component_of(root, r"D:\Games\.minecraft\PilotLauncher\runtime\java-runtime-gamma\bin\javaw.exe"),
            Some("java-runtime-gamma".to_string())
        );
        assert_eq!(runtime_component_of(root, r"C:\Program Files\Java\bin\javaw.exe"), None);
    }

    #[test]
    fn java_need_keeps() {
        let mut need = JavaNeed::default();
        need.majors.insert(8);
        need.components.insert("java-runtime-gamma".into());
        assert!(need.keeps("jre-legacy", 8));
        assert!(need.keeps("Java-Runtime-Gamma", 17));
        assert!(!need.keeps("java-runtime-alpha", 16));
    }
}
