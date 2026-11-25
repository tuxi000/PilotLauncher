//! 地图（存档）导入：从 zip 中找出 level.dat 所在目录作为存档根目录，解压到实例的 saves/。
//!
//! - 兼容多层嵌套（例如 xxx.zip/MyMap v1.2/MyMap/level.dat）与一个 zip 内有多个存档
//! - 只取最外层的存档根（存档内部若还有 level.dat，例如备份目录，不会被当成独立存档）
//! - 重名时自动改名为「名称 (2)」「名称 (3)」…
//! - 防 zip slip：条目名经过 enclosed_name 规范化，并拒绝绝对路径、盘符与 `..`

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context};
use serde::Serialize;

use crate::state::Core;
use crate::tasks::{spawn_task, TaskHandle};

/// 规范化并校验 zip 条目路径（传入 `entry.enclosed_name()`），返回以 '/' 分隔的相对路径
pub fn safe_entry_path<P: AsRef<Path>>(enclosed: Option<P>) -> Option<String> {
    let enclosed = enclosed?;
    let mut parts = Vec::new();
    for c in enclosed.as_ref().components() {
        match c {
            Component::Normal(s) => {
                let s = s.to_string_lossy().to_string();
                if s.contains(':') {
                    return None;
                }
                parts.push(s);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Windows 文件夹名清理
pub fn sanitize_dir_name(name: &str) -> String {
    const ILLEGAL: &str = "\\/:*?\"<>|";
    let s: String = name
        .chars()
        .map(|c| if ILLEGAL.contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let s = s.trim().trim_end_matches('.').trim().to_string();
    if s.is_empty() || s == ".." {
        "World".into()
    } else {
        s.chars().take(80).collect()
    }
}

pub fn unique_dir(parent: &Path, base: &str) -> (String, PathBuf) {
    let base = sanitize_dir_name(base);
    let mut name = base.clone();
    let mut n = 2;
    while parent.join(&name).exists() {
        name = format!("{base} ({n})");
        n += 1;
    }
    let path = parent.join(&name);
    (name, path)
}

/// 找出 zip 中所有存档根目录（以 '/' 结尾的前缀，"" 表示 zip 根）
pub fn find_world_roots(names: &[String]) -> Vec<String> {
    let mut roots: Vec<String> = names
        .iter()
        .filter_map(|n| {
            let (dir, file) = match n.rsplit_once('/') {
                Some((d, f)) => (format!("{d}/"), f),
                None => (String::new(), n.as_str()),
            };
            (file == "level.dat").then_some(dir)
        })
        .collect();
    roots.sort_by_key(|r| r.len());
    roots.dedup();
    let mut out: Vec<String> = Vec::new();
    for r in roots {
        if !out.iter().any(|o| r.starts_with(o.as_str())) {
            out.push(r);
        }
    }
    out
}

/// 阻塞解压：返回创建的存档名
fn extract_worlds(
    zip_path: &Path,
    saves: &Path,
    fallback_name: &str,
    task: &TaskHandle,
    stage: (u32, u32),
) -> anyhow::Result<Vec<String>> {
    let f = std::fs::File::open(zip_path).with_context(|| format!("无法打开 {}", zip_path.display()))?;
    let mut zip = zip::ZipArchive::new(f).context("不是有效的 zip 文件")?;
    let mut names = Vec::with_capacity(zip.len());
    let mut sizes = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let e = zip.by_index(i)?;
        names.push(safe_entry_path(e.enclosed_name()).unwrap_or_default());
        sizes.push(if e.is_dir() { 0 } else { e.size() });
    }
    let roots = find_world_roots(&names);
    if roots.is_empty() {
        bail!("压缩包中没有找到 level.dat，不是 Minecraft 存档");
    }
    let total_files = names
        .iter()
        .filter(|n| !n.is_empty() && roots.iter().any(|r| n.starts_with(r.as_str())))
        .count() as u64;
    let total_bytes: u64 = names
        .iter()
        .zip(sizes.iter())
        .filter(|(n, _)| !n.is_empty() && roots.iter().any(|r| n.starts_with(r.as_str())))
        .map(|(_, s)| *s)
        .sum();
    task.set_stage(stage.0, stage.1, "解压存档", total_files, total_bytes);

    std::fs::create_dir_all(saves)?;
    let mut created = Vec::new();
    for root in &roots {
        let base = if root.is_empty() {
            fallback_name.to_string()
        } else {
            root.trim_end_matches('/').rsplit('/').next().unwrap_or(fallback_name).to_string()
        };
        let (name, dest) = unique_dir(saves, &base);
        std::fs::create_dir_all(&dest)?;
        for i in 0..zip.len() {
            if task.is_cancelled() {
                let _ = std::fs::remove_dir_all(&dest);
                bail!("任务已取消");
            }
            let n = &names[i];
            if n.is_empty() || !n.starts_with(root.as_str()) {
                continue;
            }
            let rel = &n[root.len()..];
            if rel.is_empty() {
                continue;
            }
            let mut e = zip.by_index(i)?;
            let out = dest.join(rel.replace('/', "\\"));
            // 双重保险：解压目标必须位于存档目录内
            if !out.starts_with(&dest) {
                continue;
            }
            if e.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out).with_context(|| format!("写入 {} 失败", out.display()))?;
            std::io::copy(&mut e, &mut w)?;
            task.add_progress(1, sizes[i]);
        }
        created.push(name);
    }
    Ok(created)
}

pub async fn import_zip(
    task: &Arc<TaskHandle>,
    zip: &Path,
    saves: &Path,
    stage: (u32, u32),
    fallback_name: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let fallback = fallback_name
        .map(|s| s.to_string())
        .or_else(|| zip.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "World".into());
    let (t, z, s) = (task.clone(), zip.to_path_buf(), saves.to_path_buf());
    tokio::task::spawn_blocking(move || extract_worlds(&z, &s, &fallback, &t, stage)).await?
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorldImportResult {
    pub instance: String,
    pub worlds: Vec<String>,
}

/// 导入本地地图 zip 到实例（后台任务），返回任务 id
pub fn start_import(core: &Arc<Core>, path: String, instance: String) -> anyhow::Result<String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    if !p.is_file() {
        bail!("文件不存在：{}", p.display());
    }
    let dirs = core.game_dirs();
    let inst_dir = dirs.instance_dir(&instance);
    if !inst_dir.is_dir() {
        bail!("实例 {instance} 不存在");
    }
    let file_name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let task = core.tasks.create(format!("导入地图 {file_name} 到 {instance}"), "world");
    let id = task.id.clone();
    let saves = inst_dir.join("saves");
    spawn_task(task, move |task| async move {
        let created = import_zip(&task, &p, &saves, (1, 1), None).await?;
        if created.is_empty() {
            bail!("没有导入任何存档");
        }
        Ok(())
    });
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots() {
        let names: Vec<String> = [
            "Pack/Map A/level.dat",
            "Pack/Map A/region/r.0.0.mca",
            "Pack/Map A/backup/level.dat",
            "Pack/Map B/level.dat",
            "readme.txt",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(find_world_roots(&names), vec!["Pack/Map A/", "Pack/Map B/"]);
        assert_eq!(find_world_roots(&["level.dat".to_string()]), vec![""]);
        assert!(find_world_roots(&["level.dat_old".to_string()]).is_empty());
    }

    #[test]
    fn names() {
        assert_eq!(sanitize_dir_name("a:b?"), "a_b_");
        assert_eq!(sanitize_dir_name(".."), "World");
    }
}
