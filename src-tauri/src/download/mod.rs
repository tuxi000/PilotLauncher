//! 多线程下载器：并发、重试、多源回退、SHA-1 校验、进度上报。

pub mod source;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::tasks::TaskHandle;
use crate::util::file_sha1;

#[derive(Clone, Debug)]
pub struct DownloadItem {
    /// 按优先级排列的候选地址
    pub urls: Vec<String>,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verify {
    /// 只检查存在与大小（启动前快速检查）
    Quick,
    /// 同时校验 SHA-1（安装 / 修复）
    Full,
}

#[derive(Clone, Copy, Debug)]
pub struct DownloadOptions {
    pub concurrency: usize,
    pub retries: u32,
    pub verify: Verify,
}

pub fn build_client() -> anyhow::Result<reqwest::Client> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("PilotLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .pool_max_idle_per_host(64)
        .build()
        .context("创建 HTTP 客户端失败")?;
    Ok(client)
}

/// 检查本地文件是否已经完整
pub async fn is_valid(item: &DownloadItem, verify: Verify) -> bool {
    let meta = match tokio::fs::metadata(&item.path).await {
        Ok(m) if m.is_file() => m,
        _ => return false,
    };
    if let Some(size) = item.size {
        if size > 0 && meta.len() != size {
            return false;
        }
    }
    if verify == Verify::Full {
        if let Some(expected) = &item.sha1 {
            return match file_sha1(&item.path).await {
                Ok(actual) => actual.eq_ignore_ascii_case(expected),
                Err(_) => false,
            };
        }
    }
    true
}

/// 过滤出需要下载的文件（不发起网络请求）
pub async fn filter_missing(items: Vec<DownloadItem>, verify: Verify) -> Vec<DownloadItem> {
    let mut out = Vec::new();
    for item in items {
        if !is_valid(&item, verify).await {
            out.push(item);
        }
    }
    out
}

fn dedupe(items: Vec<DownloadItem>) -> Vec<DownloadItem> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|i| seen.insert(i.path.to_string_lossy().to_lowercase()))
        .collect()
}

pub fn total_size(items: &[DownloadItem]) -> u64 {
    items.iter().map(|i| i.size.unwrap_or(0)).sum()
}

/// 并发下载全部文件。任何一个文件最终失败都会返回错误（其余文件仍会尽量下完）。
pub async fn download_all(
    client: &reqwest::Client,
    items: Vec<DownloadItem>,
    opts: DownloadOptions,
    task: &Arc<TaskHandle>,
) -> anyhow::Result<()> {
    let items = dedupe(items);
    let results: Vec<Result<(), String>> = futures_util::stream::iter(items.into_iter())
        .map(|item| {
            let client = client.clone();
            let task = task.clone();
            async move { process_item(&client, item, opts, &task).await }
        })
        .buffer_unordered(opts.concurrency.max(1))
        .collect()
        .await;

    if task.is_cancelled() {
        bail!("任务已取消");
    }
    let failures: Vec<String> = results.into_iter().filter_map(|r| r.err()).collect();
    if !failures.is_empty() {
        let preview: Vec<&str> = failures.iter().take(5).map(|s| s.as_str()).collect();
        bail!(
            "{} 个文件下载失败：\n{}{}",
            failures.len(),
            preview.join("\n"),
            if failures.len() > 5 { "\n……" } else { "" }
        );
    }
    Ok(())
}

async fn process_item(
    client: &reqwest::Client,
    item: DownloadItem,
    opts: DownloadOptions,
    task: &Arc<TaskHandle>,
) -> Result<(), String> {
    if task.is_cancelled() {
        return Err("已取消".into());
    }
    let size = item.size.unwrap_or(0);
    if is_valid(&item, opts.verify).await {
        task.add_progress(1, size);
        return Ok(());
    }
    if item.urls.is_empty() {
        return Err(format!("{}：没有可用的下载地址", item.path.display()));
    }
    let mut last_err = String::new();
    for attempt in 0..=opts.retries {
        for url in &item.urls {
            let mut counted = 0u64;
            match fetch_to_file(client, url, &item, task, &mut counted).await {
                Ok(()) => {
                    // 服务器未提供大小时，以实际下载量为准
                    if size > 0 && counted != size {
                        task.sub_bytes(counted);
                        task.add_bytes(size);
                    }
                    task.add_progress(1, 0);
                    return Ok(());
                }
                Err(e) => {
                    task.sub_bytes(counted);
                    last_err = format!("{url} → {e:#}");
                    if task.is_cancelled() {
                        return Err("已取消".into());
                    }
                }
            }
        }
        if attempt < opts.retries {
            tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
        }
    }
    Err(format!("{}：{}", item.path.display(), last_err))
}

fn part_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

async fn fetch_to_file(
    client: &reqwest::Client,
    url: &str,
    item: &DownloadItem,
    task: &TaskHandle,
    counted: &mut u64,
) -> anyhow::Result<()> {
    if let Some(parent) = item.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let part = part_path(&item.path);
    let result = stream_to_file(client, url, &part, task, counted).await;
    let digest = match result {
        Ok(d) => d,
        Err(e) => {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }
    };
    if let Some(size) = item.size {
        if size > 0 && *counted != size {
            let _ = tokio::fs::remove_file(&part).await;
            bail!("文件大小不符（期望 {size}，实际 {}）", *counted);
        }
    }
    if let Some(expected) = &item.sha1 {
        if !digest.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&part).await;
            bail!("SHA-1 校验失败");
        }
    }
    if tokio::fs::metadata(&item.path).await.is_ok() {
        let _ = tokio::fs::remove_file(&item.path).await;
    }
    tokio::fs::rename(&part, &item.path)
        .await
        .with_context(|| format!("无法写入 {}", item.path.display()))?;
    Ok(())
}

async fn stream_to_file(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    task: &TaskHandle,
    counted: &mut u64,
) -> anyhow::Result<String> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(part).await?;
    let mut hasher = Sha1::new();
    let mut stream = resp.bytes_stream();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(30), stream.next())
            .await
            .map_err(|_| anyhow!("读取超时"))?;
        let chunk = match next {
            Some(c) => c?,
            None => break,
        };
        if task.is_cancelled() {
            bail!("已取消");
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        *counted += chunk.len() as u64;
        task.add_bytes(chunk.len() as u64);
    }
    file.flush().await?;
    drop(file);
    Ok(hex::encode(hasher.finalize()))
}

/// 下载小文件到内存并用 `parse` 校验/解析。依次尝试每个地址，失败重试。
pub async fn fetch_with<T, F>(
    client: &reqwest::Client,
    urls: &[String],
    retries: u32,
    parse: F,
) -> anyhow::Result<T>
where
    F: Fn(&[u8]) -> anyhow::Result<T>,
{
    let mut last: Option<anyhow::Error> = None;
    for attempt in 0..=retries {
        for url in urls {
            let resp = client
                .get(url)
                .timeout(Duration::from_secs(60))
                .send()
                .await
                .and_then(|r| r.error_for_status());
            match resp {
                Ok(resp) => match resp.bytes().await {
                    Ok(bytes) => match parse(&bytes) {
                        Ok(v) => return Ok(v),
                        Err(e) => last = Some(anyhow!("{url}：{e:#}")),
                    },
                    Err(e) => last = Some(anyhow!("{url}：{e}")),
                },
                Err(e) => last = Some(anyhow!("{url}：{e}")),
            }
        }
        if attempt < retries {
            tokio::time::sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("没有可用的下载地址")))
}

pub async fn fetch_bytes(client: &reqwest::Client, urls: &[String], retries: u32) -> anyhow::Result<Vec<u8>> {
    fetch_with(client, urls, retries, |b| Ok(b.to_vec())).await
}
