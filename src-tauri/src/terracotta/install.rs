//! 联机组件的下载、校验与解压。
//!
//! 安装位置：`<数据目录>/terracotta/<版本>/`（便携模式下数据目录是 `.minecraft/PilotLauncher/`）。
//! 流程：用现有下载器下载 tar.gz（遵守下载源设置，失败自动换源，进度显示在任务卡片）→
//! 校验整个包的 SHA-512 / SHA-256 → 解压到临时目录并逐个校验文件 SHA-512 → 改名为正式目录。
//! 文件原样写出，不做任何修改。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use sha2::{Digest, Sha256, Sha512};

use super::metadata::{self, FILES, PACKAGE_NAME, PACKAGE_SHA256, PACKAGE_SHA512, PACKAGE_SIZE, VERSION};
use crate::download::{download_all, DownloadItem, DownloadOptions, Verify};
use crate::paths::Paths;
use crate::state::Core;
use crate::tasks::{spawn_task, TaskHandle};

pub fn bundle_dir(paths: &Paths) -> PathBuf {
    paths.terracotta_dir().join(VERSION)
}

pub fn exe_path(paths: &Paths) -> PathBuf {
    bundle_dir(paths).join(metadata::EXE_NAME)
}

/// 快速检查：文件都在且大小正确（启动前会再做一次完整的 SHA-512 校验）
pub fn is_installed(paths: &Paths) -> bool {
    let dir = bundle_dir(paths);
    FILES.iter().all(|f| {
        std::fs::metadata(dir.join(f.name))
            .map(|m| m.is_file() && m.len() == f.size)
            .unwrap_or(false)
    })
}

/// 完整校验已安装的文件（阻塞，约 10 MB）
pub fn verify_installed(paths: &Paths) -> anyhow::Result<()> {
    let dir = bundle_dir(paths);
    for f in FILES {
        let bytes = std::fs::read(dir.join(f.name)).with_context(|| format!("无法读取联机组件文件 {}", f.name))?;
        if !hex::encode(Sha512::digest(&bytes)).eq_ignore_ascii_case(f.sha512) {
            bail!("联机组件文件 {} 校验失败（可能已损坏或被修改），请重新安装联机组件", f.name);
        }
    }
    Ok(())
}

/// 开始安装，返回任务 id
pub fn start_install(core: &Arc<Core>) -> anyhow::Result<String> {
    if core.terracotta.installing.swap(true, Ordering::SeqCst) {
        bail!("联机组件正在安装中");
    }
    let task = core
        .tasks
        .create(format!("安装联机组件 Terracotta {VERSION}"), "terracotta");
    let id = task.id.clone();
    core.terracotta.emit(core);
    let core2 = core.clone();
    spawn_task(task, move |task| async move {
        let result = install(&core2, &task).await;
        core2.terracotta.installing.store(false, Ordering::SeqCst);
        core2.terracotta.verified.store(false, Ordering::SeqCst);
        core2.terracotta.emit(&core2);
        result
    });
    Ok(id)
}

async fn install(core: &Arc<Core>, task: &Arc<TaskHandle>) -> anyhow::Result<()> {
    let settings = core.settings();
    // 正在运行的联机组件会占用文件，先停掉（其他启动器启动的实例不受影响）
    super::stop_owned(core).await;

    let archive = core.paths.cache_dir().join(PACKAGE_NAME);
    let labels = metadata::source_labels(settings.download_source);
    task.set_stage(
        1,
        2,
        format!("下载 Terracotta {VERSION}（{}）", labels.join(" → ")),
        1,
        PACKAGE_SIZE,
    );
    // 之前留下的同名缓存：大小对但内容不对时删掉重下
    if archive.is_file() && !archive_ok(&archive).await {
        let _ = tokio::fs::remove_file(&archive).await;
    }
    let item = DownloadItem {
        urls: metadata::download_urls(settings.download_source),
        path: archive.clone(),
        sha1: None,
        size: Some(PACKAGE_SIZE),
    };
    let opts = DownloadOptions {
        concurrency: 1,
        retries: settings.retries,
        verify: Verify::Full,
    };
    download_all(&core.http, vec![item], opts, task).await?;
    if !archive_ok(&archive).await {
        let _ = tokio::fs::remove_file(&archive).await;
        bail!("安装包哈希校验失败（文件损坏或下载源返回了错误的内容），请重试或在设置中切换下载源");
    }

    if task.is_cancelled() {
        bail!("任务已取消");
    }
    task.set_stage(2, 2, "校验并解压联机组件", 0, 0);
    let paths = core.paths.clone();
    let archive2 = archive.clone();
    tokio::task::spawn_blocking(move || extract(&paths, &archive2))
        .await
        .map_err(|e| anyhow!("解压任务异常：{e}"))??;
    let _ = tokio::fs::remove_file(&archive).await;
    cleanup_old_versions(&core.paths);
    Ok(())
}

/// 整个安装包的 SHA-512 与 SHA-256 都要一致
async fn archive_ok(path: &Path) -> bool {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> bool {
        match std::fs::read(&p) {
            Ok(bytes) => {
                (bytes.len() as u64) == PACKAGE_SIZE
                    && hex::encode(Sha512::digest(&bytes)).eq_ignore_ascii_case(PACKAGE_SHA512)
                    && hex::encode(Sha256::digest(&bytes)).eq_ignore_ascii_case(PACKAGE_SHA256)
            }
            Err(_) => false,
        }
    })
    .await
    .unwrap_or(false)
}

/// 解压到临时目录并逐个校验，全部通过后再替换正式目录
fn extract(paths: &Paths, archive: &Path) -> anyhow::Result<()> {
    let gz = std::fs::read(archive).context("读取安装包失败")?;
    let mut raw: Vec<u8> = Vec::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_end(&mut raw)
        .context("解压安装包失败（gzip 数据损坏）")?;
    let entries = tar_entries(&raw)?;

    let root = paths.terracotta_dir();
    std::fs::create_dir_all(&root)?;
    let tmp = root.join(format!(".install-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp)?;

    let written = (|| -> anyhow::Result<()> {
        for f in FILES {
            let entry = entries
                .iter()
                .find(|e| e.0.trim_start_matches("./").eq_ignore_ascii_case(f.name))
                .ok_or_else(|| anyhow!("安装包中缺少 {}", f.name))?;
            let body: &[u8] = entry.1;
            if !hex::encode(Sha512::digest(body)).eq_ignore_ascii_case(f.sha512) {
                bail!("{} 的 SHA-512 校验失败", f.name);
            }
            std::fs::write(tmp.join(f.name), body).with_context(|| format!("写入 {} 失败", f.name))?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }

    let target = bundle_dir(paths);
    if target.exists() {
        let old = root.join(format!(".old-{}", uuid::Uuid::new_v4().simple()));
        if let Err(e) = std::fs::rename(&target, &old) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(anyhow!("无法替换旧的联机组件（可能仍在运行，请先停止联机组件）：{e}"));
        }
        let _ = std::fs::remove_dir_all(&old);
    }
    std::fs::rename(&tmp, &target).with_context(|| format!("无法写入 {}", target.display()))?;
    Ok(())
}

/// 删除旧版本目录和残留的临时目录（run/ 与 owned-port 保留）
fn cleanup_old_versions(paths: &Paths) {
    let Ok(dir) = std::fs::read_dir(paths.terracotta_dir()) else {
        return;
    };
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir || name == VERSION || name == "run" {
            continue;
        }
        let looks_like_version = name.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
        if looks_like_version || name.starts_with(".old-") || name.starts_with(".install-") {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

// ───────────── 极简 tar 读取（只需要普通文件；支持 ustar 前缀、GNU 长文件名与 base-256 大小） ─────────────

fn parse_octal(field: &[u8]) -> Option<u64> {
    if let Some(first) = field.first() {
        if first & 0x80 != 0 {
            // GNU base-256
            let mut v: u64 = 0;
            for (i, b) in field.iter().enumerate() {
                let b = if i == 0 { b & 0x7f } else { *b };
                if (v >> 56) != 0 {
                    return None;
                }
                v = (v << 8) | (b as u64);
            }
            return Some(v);
        }
    }
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    let s = String::from_utf8_lossy(&field[..end]).trim().to_string();
    if s.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(&s, 8).ok()
}

fn cstr(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// 返回 (文件名, 内容) 列表
fn tar_entries(data: &[u8]) -> anyhow::Result<Vec<(String, &[u8])>> {
    let mut out: Vec<(String, &[u8])> = Vec::new();
    let mut pos: usize = 0;
    let mut long_name: Option<String> = None;
    while pos + 512 <= data.len() {
        let header = &data[pos..pos + 512];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let size = parse_octal(&header[124..136]).ok_or_else(|| anyhow!("安装包格式错误（tar 头部损坏）"))?;
        let size = usize::try_from(size).map_err(|_| anyhow!("安装包格式错误（文件过大）"))?;
        let kind = header[156];
        let mut name = cstr(&header[0..100]);
        // POSIX ustar 才有 prefix 字段（GNU 格式同位置是其他数据）
        if &header[257..263] == b"ustar\0" {
            let prefix = cstr(&header[345..500]);
            if !prefix.is_empty() {
                name = format!("{prefix}/{name}");
            }
        }
        let start = pos + 512;
        let end = start
            .checked_add(size)
            .filter(|&e| e <= data.len())
            .ok_or_else(|| anyhow!("安装包格式错误（数据被截断）"))?;
        let body = &data[start..end];
        match kind {
            b'L' => long_name = Some(cstr(body)),
            b'x' | b'g' | b'K' => {}
            b'0' | 0 | b'7' => {
                let n = long_name.take().unwrap_or(name);
                out.push((n, body));
            }
            _ => long_name = None,
        }
        pos = start + (size + 511) / 512 * 512;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octal() {
        assert_eq!(parse_octal(b"00000001750\0"), Some(1000));
        assert_eq!(parse_octal(b"           \0"), Some(0));
    }

    #[test]
    fn tar_single_file() {
        let mut data = vec![0u8; 512 * 4];
        data[..5].copy_from_slice(b"a.txt");
        data[124..136].copy_from_slice(b"00000000005\0");
        data[156] = b'0';
        data[512..517].copy_from_slice(b"hello");
        let e = tar_entries(&data).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].0, "a.txt");
        assert_eq!(e[0].1, b"hello");
    }
}
