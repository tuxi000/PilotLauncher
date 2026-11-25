//! OptiFine（版本列表与下载固定走 BMCLAPI：官方 optifine.net 需要跳转广告页，且不稳定）。
//!
//! 安装方式（参考 HMCL 的做法）：
//! - 与 Forge 共存：把 OptiFine jar 直接放进实例的 mods 文件夹，由 Forge 加载。
//! - 单独安装：用实例的 Java 运行安装器内的 `optifine.Patcher <原版 jar> <安装器> <输出>`，
//!   生成补丁后的库 `optifine:OptiFine:<mc>_<type>_<patch>`，再写入 launchwrapper 版本 JSON
//!   （mainClass = net.minecraft.launchwrapper.Launch，--tweakClass optifine.OptiFineTweaker），
//!   launchwrapper 优先使用安装器自带的 launchwrapper-of（支持 Java 9+），其次 launchwrapper-2.0，最后官方 1.12。
//!
//! 为什么不用 PCL 式的“运行官方安装器 GUI / 伪造 .minecraft 目录再复制版本”：
//! Patcher 是无界面的命令行入口，输入输出都是确定的文件路径，不需要伪造 launcher_profiles.json，
//! 也不会受版本隔离目录结构影响；失败时有明确的退出码与输出，便于给出错误提示。

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Value};

use super::{console_java, download_options, instance_java, merge_json, run_java, LoaderKind, LoaderVersion, Stager};
use crate::download::source::BMCLAPI;
use crate::download::{download_all, DownloadItem, Verify};
use crate::minecraft::library::MavenCoord;
use crate::state::Core;
use crate::tasks::TaskHandle;
use crate::util::file_sha1;

pub async fn list(core: &Core, mc: &str, force: bool) -> anyhow::Result<Vec<LoaderVersion>> {
    let url = format!("{BMCLAPI}/optifine/{mc}");
    let key = format!("optifine-{mc}");
    let data = super::meta::cached(core, &key, force, || super::meta::fetch_only(core, &url, super::meta::is_json_array))
        .await
        .context("获取 OptiFine 版本列表失败（BMCLAPI）")?;
    let items: Vec<Value> = serde_json::from_slice(data.as_slice())?;

    let mut stable = Vec::new();
    let mut preview = Vec::new();
    for it in items {
        let (Some(ty), Some(patch)) = (
            it.get("type").and_then(|v| v.as_str()),
            it.get("patch").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        let filename = it.get("filename").and_then(|v| v.as_str()).unwrap_or("");
        let is_pre = patch.starts_with("pre") || filename.starts_with("preview");
        let forge = it
            .get("forge")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && !s.contains("N/A"))
            .map(|s| format!("兼容 {s}"));
        let v = LoaderVersion {
            kind: LoaderKind::OptiFine,
            id: format!("{ty}/{patch}"),
            version: format!("{ty}_{patch}"),
            mc_version: mc.to_string(),
            stable: !is_pre,
            recommended: false,
            latest: false,
            note: forge,
        };
        if is_pre {
            preview.push(v);
        } else {
            stable.push(v);
        }
    }
    // BMCLAPI 按发布时间升序返回：反转为新到旧，正式版在前
    stable.reverse();
    preview.reverse();
    if let Some(first) = stable.first_mut() {
        first.recommended = true;
        first.latest = true;
    } else if let Some(first) = preview.first_mut() {
        first.latest = true;
    }
    stable.extend(preview);
    Ok(stable)
}

struct InstallerInfo {
    has_patcher: bool,
    launchwrapper_of: Option<String>,
    has_lw2: bool,
}

fn inspect_installer(path: &Path) -> anyhow::Result<InstallerInfo> {
    let f = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(f).context("OptiFine 安装器已损坏")?;
    let has_patcher = zip.by_name("optifine/Patcher.class").is_ok();
    let has_lw2 = zip.by_name("launchwrapper-2.0.jar").is_ok();
    let launchwrapper_of = match zip.by_name("launchwrapper-of.txt") {
        Ok(mut e) => {
            let mut s = String::new();
            e.read_to_string(&mut s)?;
            Some(s.trim().to_string()).filter(|s| !s.is_empty())
        }
        Err(_) => None,
    };
    if !has_patcher && zip.by_name("Config.class").is_err() && zip.by_name("net/optifine/Config.class").is_err() {
        bail!("下载的文件不是 OptiFine 安装器");
    }
    Ok(InstallerInfo { has_patcher, launchwrapper_of, has_lw2 })
}

fn extract_to(zip_path: &Path, entry: &str, dest: &Path) -> anyhow::Result<()> {
    let f = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(f)?;
    let mut e = zip.by_name(entry).with_context(|| format!("安装器中没有 {entry}"))?;
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut out = std::fs::File::create(dest)?;
    std::io::copy(&mut e, &mut out)?;
    Ok(())
}

async fn local_lib(name: &str, path: &Path, rel: &str) -> anyhow::Result<Value> {
    let sha1 = file_sha1(path).await?;
    let size = std::fs::metadata(path)?.len();
    // url 为空：启动前检查只要求文件存在，缺失时提示重新安装（不会去错误的地址下载）
    Ok(json!({
        "name": name,
        "downloads": { "artifact": { "path": rel, "url": "", "sha1": sha1, "size": size } }
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn install(
    core: &Arc<Core>,
    task: &Arc<TaskHandle>,
    st: &mut Stager,
    name: &str,
    mc: &str,
    id: &str,
    with_forge: bool,
    json: &mut Value,
) -> anyhow::Result<()> {
    let (ty, patch) = id.split_once('/').ok_or_else(|| anyhow!("无效的 OptiFine 版本：{id}"))?;
    let tag = format!("{mc}_{ty}_{patch}");
    let dirs = core.game_dirs();
    let installer = core.paths.cache_dir().join("optifine").join(format!("OptiFine_{tag}.jar"));

    st.next(format!("下载 OptiFine {ty}_{patch}（BMCLAPI）"), 1, 0);
    let item = DownloadItem {
        urls: vec![format!("{BMCLAPI}/optifine/{mc}/{ty}/{patch}")],
        path: installer.clone(),
        sha1: None,
        size: None,
    };
    download_all(&core.http, vec![item], download_options(core, Verify::Quick), task).await?;
    let p2 = installer.clone();
    let info = match tokio::task::spawn_blocking(move || inspect_installer(&p2)).await? {
        Ok(i) => i,
        Err(e) => {
            let _ = std::fs::remove_file(&installer);
            return Err(e);
        }
    };

    if with_forge {
        // 与 Forge 共存：作为 mod 放入 mods 文件夹（移除旧的 OptiFine）
        let mods = dirs.instance_dir(name).join("mods");
        std::fs::create_dir_all(&mods)?;
        if let Ok(rd) = std::fs::read_dir(&mods) {
            for e in rd.filter_map(|e| e.ok()) {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("OptiFine_") && (n.ends_with(".jar") || n.ends_with(".jar.disabled")) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        std::fs::copy(&installer, mods.join(format!("OptiFine_{tag}.jar"))).context("复制 OptiFine 到 mods 失败")?;
        task.add_progress(1, 0);
        return Ok(());
    }

    // 单独安装：Patcher + launchwrapper
    let stage = st.next("准备 Java 运行时", 0, 0);
    let java_info = instance_java(core, task, name, stage).await?;
    let java = console_java(&java_info);

    st.next("生成 OptiFine 补丁文件", 1, 0);
    let lib_dir = dirs.libraries();
    let of_name = format!("optifine:OptiFine:{tag}");
    let of_rel = MavenCoord::parse(&of_name).ok_or_else(|| anyhow!("无效的坐标"))?.path();
    let of_path = lib_dir.join(&of_rel);
    if let Some(p) = of_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let client_jar = dirs.version_jar(name);
    if !client_jar.is_file() {
        bail!("缺少原版客户端 {}", client_jar.display());
    }
    if info.has_patcher {
        let args = vec![
            "-cp".to_string(),
            installer.display().to_string(),
            "optifine.Patcher".to_string(),
            client_jar.display().to_string(),
            installer.display().to_string(),
            of_path.display().to_string(),
        ];
        let tmp = core.paths.cache_dir().join("installer-tmp");
        run_java(&java, &args, &tmp, task).await.context("OptiFine Patcher 运行失败")?;
        if !of_path.is_file() {
            bail!("OptiFine Patcher 没有生成 {}", of_path.display());
        }
    } else {
        // 很旧的 OptiFine 没有 Patcher，本身就是完整的库
        std::fs::copy(&installer, &of_path)?;
    }

    let mut libs = vec![local_lib(&of_name, &of_path, &of_rel).await?];
    let mut has_lw = false;
    if let Some(v) = &info.launchwrapper_of {
        let n = format!("optifine:launchwrapper-of:{v}");
        let rel = MavenCoord::parse(&n).ok_or_else(|| anyhow!("无效的坐标"))?.path();
        let dest = lib_dir.join(&rel);
        let (src, d2, entry) = (installer.clone(), dest.clone(), format!("launchwrapper-of-{v}.jar"));
        if tokio::task::spawn_blocking(move || extract_to(&src, &entry, &d2)).await?.is_ok() {
            libs.push(local_lib(&n, &dest, &rel).await?);
            has_lw = true;
        }
    }
    if !has_lw && info.has_lw2 {
        let n = "optifine:launchwrapper:2.0".to_string();
        let rel = MavenCoord::parse(&n).ok_or_else(|| anyhow!("无效的坐标"))?.path();
        let dest = lib_dir.join(&rel);
        let (src, d2) = (installer.clone(), dest.clone());
        if tokio::task::spawn_blocking(move || extract_to(&src, "launchwrapper-2.0.jar", &d2)).await?.is_ok() {
            libs.push(local_lib(&n, &dest, &rel).await?);
            has_lw = true;
        }
    }
    if !has_lw {
        libs.push(json!({ "name": "net.minecraft:launchwrapper:1.12" }));
    }

    let child = json!({
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "libraries": libs,
    });
    *json = merge_json(json, &child);
    let legacy_args = json.get("minecraftArguments").and_then(|a| a.as_str()).map(|s| s.to_string());
    match legacy_args {
        Some(a) => {
            if !a.contains("OptiFineTweaker") {
                json["minecraftArguments"] = Value::String(format!("{a} --tweakClass optifine.OptiFineTweaker"));
            }
        }
        None => {
            if !json.get("arguments").map(|a| a.is_object()).unwrap_or(false) {
                json["arguments"] = json!({});
            }
            let mut game = json["arguments"].get("game").and_then(|g| g.as_array()).cloned().unwrap_or_default();
            game.push(Value::String("--tweakClass".into()));
            game.push(Value::String("optifine.OptiFineTweaker".into()));
            json["arguments"]["game"] = Value::Array(game);
        }
    }
    task.add_progress(1, 0);
    Ok(())
}
