//! 原版安装与文件补全。

use std::sync::Arc;

use anyhow::{anyhow, bail, Context};

use super::assets::{ensure_index, object_items};
use super::library::resolve_libraries;
use super::manifest::get_manifest;
use super::rules::Features;
use super::version::load_resolved;
use crate::download::{download_all, fetch_with, filter_missing, total_size, DownloadItem, DownloadOptions, Verify};
use crate::instance;
use crate::java::runtime::ensure_runtime;
use crate::state::Core;
use crate::tasks::TaskHandle;
use crate::util::sha1_hex;

/// 收集实例运行所需的全部文件（客户端 jar、库、natives、资源）。
/// 会在需要时下载资源索引。
pub async fn collect_game_files(core: &Core, name: &str) -> anyhow::Result<Vec<DownloadItem>> {
    let dirs = core.game_dirs();
    let version = load_resolved(&dirs, name)?;
    let sources = |u: &str| core.sources(u);
    let mut items = Vec::new();

    // 客户端 jar
    let jar_id = version.jar_id();
    let jar_path = dirs.version_jar(&jar_id);
    match version.downloads.as_ref().and_then(|d| d.get("client")) {
        Some(client) => items.push(DownloadItem {
            urls: sources(client.url.as_str()),
            path: jar_path,
            sha1: client.sha1.clone(),
            size: client.size,
        }),
        None => {
            if !jar_path.is_file() {
                bail!("版本 JSON 中没有客户端下载地址，且本地缺少 {}", jar_path.display());
            }
        }
    }

    // 依赖库（库的 rules 只涉及操作系统，与启动特性无关）
    let libs = resolve_libraries(&version, &dirs.libraries(), &Features::default(), &sources);
    for lib in libs {
        match lib.download {
            Some(d) => items.push(d),
            None => {
                if !lib.path.is_file() {
                    bail!("缺少依赖库且没有下载地址：{}", lib.path.display());
                }
            }
        }
    }

    // 资源
    if let Some(idx) = &version.asset_index {
        let index = ensure_index(core, idx, &dirs.assets()).await?;
        items.extend(object_items(&index, &dirs.assets(), &sources));
    }
    Ok(items)
}

fn options(core: &Core, verify: Verify) -> DownloadOptions {
    let s = core.settings();
    DownloadOptions { concurrency: s.concurrency, retries: s.retries, verify }
}

/// 下载/校验实例文件
pub async fn complete_files(
    core: &Core,
    task: &Arc<TaskHandle>,
    name: &str,
    verify: Verify,
    stage: (u32, u32),
) -> anyhow::Result<()> {
    task.set_stage(stage.0, stage.1, "分析游戏文件", 0, 0);
    let items = collect_game_files(core, name).await?;
    let total = total_size(&items);
    task.set_stage(stage.0, stage.1, "下载游戏文件", items.len() as u64, total);
    download_all(&core.http, items, options(core, verify), task).await
}

/// 启动前快速检查：只返回缺失或大小不符的文件
pub async fn missing_files_quick(core: &Core, name: &str) -> anyhow::Result<Vec<DownloadItem>> {
    let items = collect_game_files(core, name).await?;
    Ok(filter_missing(items, Verify::Quick).await)
}

pub async fn download_items(core: &Core, task: &Arc<TaskHandle>, items: Vec<DownloadItem>) -> anyhow::Result<()> {
    let total = total_size(&items);
    task.set_stage(1, 1, "补全游戏文件", items.len() as u64, total);
    download_all(&core.http, items, options(core, Verify::Full), task).await
}

/// 下载原版版本 JSON（校验 SHA-1），返回 JSON 值（id 仍为原版号）
pub async fn fetch_version_json(core: &Core, mc_version: &str) -> anyhow::Result<serde_json::Value> {
    let settings = core.settings();
    let manifest = get_manifest(core, false).await?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == mc_version)
        .cloned()
        .ok_or_else(|| anyhow!("版本清单中找不到 {mc_version}"))?;
    let expected = entry.sha1.clone();
    let bytes = fetch_with(&core.http, &core.sources(&entry.url), settings.retries, |b| {
        if let Some(e) = &expected {
            if !sha1_hex(b).eq_ignore_ascii_case(e) {
                bail!("版本 JSON SHA-1 校验失败");
            }
        }
        serde_json::from_slice::<serde_json::Value>(b).context("版本 JSON 格式错误")?;
        Ok(b.to_vec())
    })
    .await
    .with_context(|| format!("下载 {mc_version} 版本 JSON 失败"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// 安装原版的三个阶段（版本 JSON → 文件 → Java 运行时），阶段序号从 `first` 开始，共 `total` 个阶段
pub async fn install_vanilla_staged(
    core: &Core,
    task: &Arc<TaskHandle>,
    mc_version: &str,
    name: &str,
    first: u32,
    total: u32,
) -> anyhow::Result<()> {
    let dirs = core.game_dirs();
    let settings = core.settings();

    task.set_stage(first, total, "获取版本信息", 1, 0);
    let mut value = fetch_version_json(core, mc_version).await?;
    // 实例名即版本 id，实现版本隔离；原始版本号另存在 clientVersion 中
    value["id"] = serde_json::Value::String(name.to_string());
    value["clientVersion"] = serde_json::Value::String(mc_version.to_string());
    let json_path = dirs.version_json(name);
    if let Some(parent) = json_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&json_path, serde_json::to_vec_pretty(&value)?).await?;
    // 原版备份：之后安装 / 更换加载器时以它为基础
    let _ = crate::util::write_json_atomic(&dirs.instance_dir(name).join(crate::modloader::VANILLA_BACKUP), &value);
    task.add_progress(1, 0);

    complete_files(core, task, name, Verify::Full, (first + 1, total)).await?;

    // Java：未设置全局 / 实例覆盖时，预先下载官方运行时
    let meta = instance::load_meta(&dirs, name)?;
    if settings.java_path.is_none() && meta.java_path.is_none() {
        let version = load_resolved(&dirs, name)?;
        let (component, major) = version.java_requirement();
        ensure_runtime(core, task, component.as_deref(), major, (first + 2, total)).await?;
    } else {
        task.set_stage(first + 2, total, "使用自定义 Java，跳过运行时下载", 0, 0);
    }

    instance::set_installed(&dirs, name, true)?;
    Ok(())
}

/// 安装原版：版本 JSON → 文件 → Java 运行时
pub async fn install_vanilla(
    core: Arc<Core>,
    task: Arc<TaskHandle>,
    mc_version: String,
    name: String,
) -> anyhow::Result<()> {
    install_vanilla_staged(&core, &task, &mc_version, &name, 1, 3).await
}

/// 修复：完整校验所有文件
pub async fn repair(core: Arc<Core>, task: Arc<TaskHandle>, name: String) -> anyhow::Result<()> {
    complete_files(&core, &task, &name, Verify::Full, (1, 1)).await?;
    let dirs = core.game_dirs();
    if instance::meta_path(&dirs, &name).is_file() {
        instance::set_installed(&dirs, &name, true)?;
    }
    Ok(())
}
