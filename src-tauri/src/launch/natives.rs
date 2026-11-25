//! natives 解压。

use std::fs::File;
use std::path::Path;

use anyhow::Context;

use crate::minecraft::library::{ExtractKind, ResolvedLibrary};

/// 清空并重新解压 natives 目录（每次启动执行，避免残留旧文件）
pub fn extract_natives(libs: &[ResolvedLibrary], dir: &Path) -> anyhow::Result<()> {
    if dir.exists() {
        // 文件被占用（例如另一个游戏进程仍在运行）时忽略删除失败，后续覆盖写入
        let _ = std::fs::remove_dir_all(dir);
    }
    std::fs::create_dir_all(dir)?;
    for lib in libs {
        match &lib.extract {
            ExtractKind::None => {}
            ExtractKind::Legacy(exclude) => extract_jar(&lib.path, dir, false, exclude)?,
            ExtractKind::FlatDll => extract_jar(&lib.path, dir, true, &[])?,
        }
    }
    Ok(())
}

fn extract_jar(jar: &Path, dir: &Path, flat_dll: bool, exclude: &[String]) -> anyhow::Result<()> {
    let file = File::open(jar).with_context(|| format!("无法打开 natives 库 {}", jar.display()))?;
    let mut zip = zip::ZipArchive::new(file).with_context(|| format!("natives 库已损坏：{}", jar.display()))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if name.starts_with('/') || name.split('/').any(|c| c == "..") {
            continue;
        }
        if name.starts_with("META-INF/") || exclude.iter().any(|e| name.starts_with(e.as_str())) {
            continue;
        }
        let out = if flat_dll {
            let lower = name.to_ascii_lowercase();
            if !lower.ends_with(".dll") || lower.contains("/x86/") || lower.contains("/arm64/") {
                continue;
            }
            match name.rsplit('/').next() {
                Some(f) if !f.is_empty() => dir.join(f),
                _ => continue,
            }
        } else {
            dir.join(&name)
        };
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut writer = match File::create(&out) {
            Ok(w) => w,
            // 文件被占用时保留现有文件
            Err(_) if out.is_file() => continue,
            Err(e) => return Err(e).with_context(|| format!("写入 {} 失败", out.display())),
        };
        std::io::copy(&mut entry, &mut writer)?;
    }
    Ok(())
}
