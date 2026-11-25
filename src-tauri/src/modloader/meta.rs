//! 加载器版本列表等「小元数据」的请求与缓存。
//!
//! 为什么需要它（第三阶段对“版本列表加载太慢”的分析）：
//! 1. 之前每次打开安装弹窗、每次点开一张卡片都会重新请求，没有任何缓存；
//! 2. 只有点开卡片才开始请求，且 Forge 先等版本列表、再串行请求推荐版信息；
//! 3. “官方优先”模式下先连官方（国内常常要等到 15 s 连接超时 / 60 s 读超时，再加重试），失败后才换镜像；
//!    NeoForge 官方接口甚至没有镜像候选，要重试 3 次才回退 BMCLAPI；
//! 4. Forge 官方 maven-metadata.xml 包含所有 Minecraft 版本（数百 KB），Fabric / Quilt 按 mc 版本请求的
//!    /versions/loader/<mc> 每一项都带完整的 launcherMeta，也有几百 KB，而且每换一个游戏版本就重新下载一遍；
//! 5. 同一时刻的重复请求（例如快速切换卡片）各发各的。
//!
//! 这里提供：
//! - 内存 + 磁盘缓存（`<数据目录>/cache/loader-meta/`，TTL 1 小时）；网络失败时使用过期缓存兜底；
//! - 同一个 key 同时只有一个请求在飞，后来者等待并复用结果（请求合并）；
//! - 按下载源取数据：只用官方 → 只请求官方；BMCLAPI → 镜像优先、失败换官方；
//!   官方优先 → 官方与镜像**同时**请求，谁先成功用谁。

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, bail, Context};

use crate::download::source;
use crate::settings::DownloadSource;
use crate::state::Core;

/// 缓存有效期
pub const TTL: Duration = Duration::from_secs(3600);
/// 只用官方源时的单次超时
const OFFICIAL_ONLY_TIMEOUT: Duration = Duration::from_secs(30);
/// 与镜像同时请求时官方的单次超时
const OFFICIAL_RACE_TIMEOUT: Duration = Duration::from_secs(20);
const MIRROR_TIMEOUT: Duration = Duration::from_secs(15);

struct Entry {
    at: Instant,
    data: Arc<Vec<u8>>,
}

#[derive(Default)]
struct MetaCache {
    mem: Mutex<HashMap<String, Entry>>,
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

fn cache() -> &'static MetaCache {
    static CACHE: OnceLock<MetaCache> = OnceLock::new();
    CACHE.get_or_init(MetaCache::default)
}

fn mem_get(key: &str, ttl: Duration) -> Option<Arc<Vec<u8>>> {
    let m = cache().mem.lock().unwrap_or_else(|e| e.into_inner());
    m.get(key).filter(|e| e.at.elapsed() < ttl).map(|e| e.data.clone())
}

fn mem_put(key: &str, at: Instant, data: Arc<Vec<u8>>) {
    let mut m = cache().mem.lock().unwrap_or_else(|e| e.into_inner());
    m.insert(key.to_string(), Entry { at, data });
}

fn key_lock(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut l = cache().locks.lock().unwrap_or_else(|e| e.into_inner());
    l.entry(key.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

fn disk_path(core: &Core, key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    core.paths.cache_dir().join("loader-meta").join(format!("{safe}.cache"))
}

/// 读取磁盘缓存：(内容, 已存在多久)
fn disk_read(path: &Path) -> Option<(Vec<u8>, Duration)> {
    let meta = std::fs::metadata(path).ok()?;
    let age = meta
        .modified()
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .unwrap_or(Duration::MAX);
    let data = std::fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }
    Some((data, age))
}

fn disk_write(path: &Path, data: &[u8]) {
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// 带缓存地获取数据。`fetch` 只在缓存失效时调用，且应当只返回已经校验过的数据（不合格的数据不会写入缓存）。
/// `force` 为 true 时跳过新鲜缓存（仍然在网络失败时使用旧缓存兜底）。
pub async fn cached<F, Fut>(core: &Core, key: &str, force: bool, fetch: F) -> anyhow::Result<Arc<Vec<u8>>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<Vec<u8>>>,
{
    if !force {
        if let Some(d) = mem_get(key, TTL) {
            return Ok(d);
        }
    }
    // 请求合并：同一个 key 同时只有一个请求，其余调用在这里等待
    let lock = key_lock(key);
    let _guard = lock.lock().await;
    // 等锁期间别的调用可能已经拿到了结果（强制刷新时只复用 5 秒内的结果）
    let fresh = if force { Duration::from_secs(5) } else { TTL };
    if let Some(d) = mem_get(key, fresh) {
        return Ok(d);
    }

    let path = disk_path(core, key);
    let disk = disk_read(&path);
    if !force {
        if let Some((data, age)) = &disk {
            if *age < TTL {
                let arc = Arc::new(data.clone());
                if let Some(at) = Instant::now().checked_sub(*age) {
                    mem_put(key, at, arc.clone());
                }
                return Ok(arc);
            }
        }
    }

    match fetch().await {
        Ok(bytes) => {
            let arc = Arc::new(bytes);
            mem_put(key, Instant::now(), arc.clone());
            disk_write(&path, arc.as_slice());
            Ok(arc)
        }
        Err(e) => match disk {
            Some((data, _)) => {
                eprintln!("[meta] {key} 请求失败，使用过期缓存：{e:#}");
                let arc = Arc::new(data);
                // 过期缓存在内存里再用 2 分钟，避免短时间内反复等待超时
                let at = Instant::now()
                    .checked_sub(TTL.saturating_sub(Duration::from_secs(120)))
                    .unwrap_or_else(Instant::now);
                mem_put(key, at, arc.clone());
                Ok(arc)
            }
            None => Err(e),
        },
    }
}

/// 两个请求同时进行，返回先成功的那个；都失败时合并错误信息
pub async fn first_ok<T, A, B>(a: A, b: B) -> anyhow::Result<T>
where
    A: Future<Output = anyhow::Result<T>>,
    B: Future<Output = anyhow::Result<T>>,
{
    tokio::pin!(a);
    tokio::pin!(b);
    let mut a_err: Option<anyhow::Error> = None;
    let mut b_err: Option<anyhow::Error> = None;
    loop {
        tokio::select! {
            r = &mut a, if a_err.is_none() => match r {
                Ok(v) => return Ok(v),
                Err(e) => a_err = Some(e),
            },
            r = &mut b, if b_err.is_none() => match r {
                Ok(v) => return Ok(v),
                Err(e) => b_err = Some(e),
            },
            else => break,
        }
    }
    let ea = a_err.map(|e| format!("{e:#}")).unwrap_or_default();
    let eb = b_err.map(|e| format!("{e:#}")).unwrap_or_default();
    bail!("{ea}\n{eb}")
}

/// 请求单个地址（失败重试 1 次；4xx 不重试），并用 `validate` 校验内容
async fn try_url<V>(core: &Core, url: &str, timeout: Duration, validate: &V) -> anyhow::Result<Vec<u8>>
where
    V: Fn(&[u8]) -> anyhow::Result<()> + Sync,
{
    let mut last: Option<anyhow::Error> = None;
    for attempt in 0..2u32 {
        match core.http.get(url).timeout(timeout).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    match resp.bytes().await {
                        Ok(b) => {
                            validate(&b[..]).with_context(|| format!("{url} 返回的数据格式不正确"))?;
                            return Ok(b.to_vec());
                        }
                        Err(e) => last = Some(anyhow!("{url}：{e}")),
                    }
                } else {
                    last = Some(anyhow!("{url}：HTTP {status}"));
                    if status.is_client_error() {
                        break;
                    }
                }
            }
            Err(e) => last = Some(anyhow!("{url}：{e}")),
        }
        if attempt == 0 {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("{url}：请求失败")))
}

/// 按下载源设置请求官方地址 `official`（镜像地址为 `mirror`）：
/// 只用官方 → 官方；BMCLAPI → 镜像优先、失败换官方；官方优先 → 两边同时请求，谁先成功用谁。
pub async fn fetch_pair<V>(core: &Core, official: &str, mirror: Option<&str>, validate: V) -> anyhow::Result<Vec<u8>>
where
    V: Fn(&[u8]) -> anyhow::Result<()> + Send + Sync,
{
    let validate = &validate;
    let source = core.settings().download_source;
    match (source, mirror) {
        (DownloadSource::Official, _) | (_, None) => try_url(core, official, OFFICIAL_ONLY_TIMEOUT, validate).await,
        (DownloadSource::Bmclapi, Some(m)) => match try_url(core, m, MIRROR_TIMEOUT, validate).await {
            Ok(b) => Ok(b),
            Err(e) => try_url(core, official, OFFICIAL_RACE_TIMEOUT, validate)
                .await
                .map_err(|e2| anyhow!("{e:#}\n{e2:#}")),
        },
        (DownloadSource::OfficialFirst, Some(m)) => {
            first_ok(
                try_url(core, official, OFFICIAL_RACE_TIMEOUT, validate),
                try_url(core, m, MIRROR_TIMEOUT, validate),
            )
            .await
        }
    }
}

/// 官方地址按 BMCLAPI 替换规则自动得到镜像地址（没有镜像规则时只请求官方）
pub async fn fetch_meta<V>(core: &Core, official: &str, validate: V) -> anyhow::Result<Vec<u8>>
where
    V: Fn(&[u8]) -> anyhow::Result<()> + Send + Sync,
{
    let mirror = source::to_mirror(official);
    fetch_pair(core, official, mirror.as_deref(), validate).await
}

/// 只请求这一个地址（例如 BMCLAPI 专有接口），不区分下载源
pub async fn fetch_only<V>(core: &Core, url: &str, validate: V) -> anyhow::Result<Vec<u8>>
where
    V: Fn(&[u8]) -> anyhow::Result<()> + Send + Sync,
{
    try_url(core, url, MIRROR_TIMEOUT, &validate).await
}

/// 校验：内容是合法 JSON
pub fn is_json(b: &[u8]) -> anyhow::Result<()> {
    serde_json::from_slice::<serde_json::Value>(b)?;
    Ok(())
}

/// 校验：内容是 JSON 数组
pub fn is_json_array(b: &[u8]) -> anyhow::Result<()> {
    let v: serde_json::Value = serde_json::from_slice(b)?;
    if !v.is_array() {
        bail!("不是 JSON 数组");
    }
    Ok(())
}

/// 校验：Forge maven-metadata.xml
pub fn is_maven_metadata(b: &[u8]) -> anyhow::Result<()> {
    let text = String::from_utf8_lossy(b);
    if !text.contains("<versioning>") {
        bail!("不是 maven-metadata.xml");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn race_prefers_first_success() {
        let slow_ok = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            anyhow::Ok(1)
        };
        let fast_err = async { Err::<i32, anyhow::Error>(anyhow!("x")) };
        assert_eq!(first_ok(fast_err, slow_ok).await.unwrap(), 1);
        let a = async { Err::<i32, anyhow::Error>(anyhow!("a")) };
        let b = async { Err::<i32, anyhow::Error>(anyhow!("b")) };
        assert!(first_ok(a, b).await.is_err());
    }
}
