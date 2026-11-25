//! Terracotta | 陶瓦联机 集成。
//!
//! - 本启动器只通过 Terracotta 提供的本机 HTTP 接口与**未修改**的官方程序交互
//!   （AGPL-3.0-or-later 的例外条款第 2 条），并在联机页与设置页明显处标注版权信息。
//! - 启动：`terracotta-<版本>.exe --hmcl2 <文件>`（Windows）。HMCL 用的是 `--hmcl <文件>`：
//!   那只是一个引导进程，它会以 DETACHED_PROCESS 另起一个 `--hmcl2 <文件>` 工作进程后自行退出。
//!   直接运行工作模式可以让陶瓦成为本启动器的子进程，便于检测意外退出与在退出时清理（kill_on_drop）。
//!   HTTP 服务就绪后，陶瓦把 `{"port": 端口}` 写入 `<文件>.tmp` 再改名为 `<文件>`。
//! - 单实例：陶瓦在 `%TEMP%\terracotta\terracotta.lock` 里记录正在运行的实例端口，
//!   HMCL / PCL CE 已启动陶瓦时会直接复用（此时不由本启动器关闭）。
//! - 状态通过 `terracotta-state` 事件推送给前端；只在联机页打开或房间进行中时轮询 `/state`。

pub mod client;
pub mod install;
pub mod metadata;

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context};
use serde::Serialize;
use tauri::Emitter;

use crate::paths::Paths;
use crate::state::Core;
pub use client::{normalize_room_code, RoomState};

pub const STATE_EVENT: &str = "terracotta-state";

/// 连续多少次请求失败后认为联机组件已退出
const MAX_FAILS: u32 = 4;

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Stopped,
    Starting,
    Running,
    Crashed(String),
}

struct Inner {
    phase: Phase,
    port: Option<u16>,
    /// 由本启动器启动（退出时负责关闭）
    owned: bool,
    child: Option<tokio::process::Child>,
    room: Option<RoomState>,
    meta: Option<client::Meta>,
    fails: u32,
}

pub struct Manager {
    inner: Mutex<Inner>,
    /// 启动 / 停止互斥
    op_lock: tokio::sync::Mutex<()>,
    pub(crate) installing: AtomicBool,
    /// 本次运行中是否已经完整校验过已安装的文件
    pub(crate) verified: AtomicBool,
    /// 联机页是否打开
    watching: AtomicBool,
    /// 轮询任务的代号（启动新的轮询时旧的自动退出）
    poll_gen: AtomicU64,
    http: reqwest::Client,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TerracottaView {
    pub installed: bool,
    pub installing: bool,
    /// 固定使用的版本
    pub version: String,
    pub package_size: u64,
    /// 按当前下载源设置排列的下载渠道
    pub sources: Vec<String>,
    pub install_dir: String,
    /// stopped / starting / running / crashed
    pub process: String,
    pub error: Option<String>,
    /// 正在复用其他程序启动的陶瓦（退出时不关闭）
    pub external: bool,
    pub port: Option<u16>,
    /// 实际运行的陶瓦版本（来自 /meta）
    pub core_version: Option<String>,
    pub easytier_version: Option<String>,
    pub room: Option<RoomState>,
    pub repo_url: String,
    pub license_url: String,
    pub easytier_url: String,
    pub docs_url: String,
}

impl Manager {
    pub fn new() -> anyhow::Result<Self> {
        // 只访问 127.0.0.1：不走系统代理，超时要短
        let http = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .context("创建联机组件 HTTP 客户端失败")?;
        Ok(Self {
            inner: Mutex::new(Inner {
                phase: Phase::Stopped,
                port: None,
                owned: false,
                child: None,
                room: None,
                meta: None,
                fails: 0,
            }),
            op_lock: tokio::sync::Mutex::new(()),
            installing: AtomicBool::new(false),
            verified: AtomicBool::new(false),
            watching: AtomicBool::new(false),
            poll_gen: AtomicU64::new(0),
            http,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn view(&self, core: &Core) -> TerracottaView {
        let installed = install::is_installed(&core.paths);
        let source = core.settings().download_source;
        let g = self.lock();
        let (process, error) = match &g.phase {
            Phase::Stopped => ("stopped", None),
            Phase::Starting => ("starting", None),
            Phase::Running => ("running", None),
            Phase::Crashed(e) => ("crashed", Some(e.clone())),
        };
        TerracottaView {
            installed,
            installing: self.installing.load(Ordering::SeqCst),
            version: metadata::VERSION.to_string(),
            package_size: metadata::PACKAGE_SIZE,
            sources: metadata::source_labels(source),
            install_dir: install::bundle_dir(&core.paths).display().to_string(),
            process: process.to_string(),
            error,
            external: g.port.is_some() && !g.owned,
            port: g.port,
            core_version: g.meta.as_ref().map(|m| m.version.clone()).filter(|s| !s.is_empty()),
            easytier_version: g.meta.as_ref().map(|m| m.easytier.clone()).filter(|s| !s.is_empty()),
            room: g.room.clone(),
            repo_url: metadata::REPO_URL.to_string(),
            license_url: metadata::LICENSE_URL.to_string(),
            easytier_url: metadata::EASYTIER_URL.to_string(),
            docs_url: metadata::DOCS_URL.to_string(),
        }
    }

    pub fn emit(&self, core: &Core) {
        let v = self.view(core);
        let _ = core.app.emit(STATE_EVENT, v);
    }

    pub fn set_watching(&self, watching: bool) {
        self.watching.store(watching, Ordering::SeqCst);
    }

    fn running_port(&self) -> Option<u16> {
        let g = self.lock();
        let port = if g.phase == Phase::Running {
            g.port
        } else {
            None
        };
        port
    }

    /// 启动器退出时调用（同步）：先让陶瓦回到待机（关闭房间、结束 EasyTier），再正常退出，最后结束子进程
    pub fn shutdown_blocking(&self, paths: &Paths) {
        self.poll_gen.fetch_add(1, Ordering::SeqCst);
        let (port, owned, child) = {
            let mut g = self.lock();
            let port = g.port.take();
            let owned = g.owned;
            let child = g.child.take();
            g.phase = Phase::Stopped;
            g.room = None;
            (port, owned, child)
        };
        if let Some(port) = port {
            if owned || child.is_some() {
                client::raw_get(port, "/state/ide");
                std::thread::sleep(Duration::from_millis(250));
                client::raw_get(port, "/panic?peaceful=true");
                clear_owned_port(paths);
            }
        }
        if let Some(mut c) = child {
            std::thread::sleep(Duration::from_millis(150));
            let _ = c.start_kill();
        }
    }
}

// ───────────────────────── 端口记录 ─────────────────────────

fn owned_port_file(paths: &Paths) -> PathBuf {
    paths.terracotta_dir().join("owned-port")
}

fn read_owned_port(paths: &Paths) -> Option<u16> {
    std::fs::read_to_string(owned_port_file(paths))
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
}

fn write_owned_port(paths: &Paths, port: u16) {
    let _ = std::fs::create_dir_all(paths.terracotta_dir());
    let _ = std::fs::write(owned_port_file(paths), port.to_string());
}

fn clear_owned_port(paths: &Paths) {
    let _ = std::fs::remove_file(owned_port_file(paths));
}

/// 陶瓦的全局锁文件里记录的端口（已有实例在运行时才有意义）
fn lock_file_port() -> Option<u16> {
    let lock = std::env::temp_dir().join("terracotta").join("terracotta.lock");
    let bytes = std::fs::read(lock).ok()?;
    if bytes.len() < 2 {
        return None;
    }
    let port = u16::from_be_bytes([bytes[0], bytes[1]]);
    if port == 0 {
        None
    } else {
        Some(port)
    }
}

fn parse_port_file(text: &str) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let p = v.get("port")?.as_u64()?;
    u16::try_from(p).ok().filter(|p| *p != 0)
}

// ───────────────────────── 启动 / 停止 ─────────────────────────

/// 确保联机组件在运行
pub async fn start(core: &Arc<Core>) -> anyhow::Result<()> {
    let m = &core.terracotta;
    let _op = m.op_lock.lock().await;
    if m.running_port().is_some() {
        return Ok(());
    }
    if m.installing.load(Ordering::SeqCst) {
        bail!("联机组件正在安装，请稍候");
    }
    if !install::is_installed(&core.paths) {
        bail!("联机组件尚未安装");
    }
    if !m.verified.load(Ordering::SeqCst) {
        let paths = core.paths.clone();
        tokio::task::spawn_blocking(move || install::verify_installed(&paths))
            .await
            .map_err(|e| anyhow!("校验任务异常：{e}"))??;
        m.verified.store(true, Ordering::SeqCst);
    }
    {
        let mut g = m.lock();
        g.phase = Phase::Starting;
        g.room = None;
        g.fails = 0;
    }
    m.emit(core);
    match start_inner(core).await {
        Ok(()) => {
            ensure_poller(core);
            refresh_now(core).await;
            m.emit(core);
            Ok(())
        }
        Err(e) => {
            let child = {
                let mut g = m.lock();
                g.phase = Phase::Crashed(format!("{e:#}"));
                g.port = None;
                g.room = None;
                g.child.take()
            };
            if let Some(mut c) = child {
                let _ = c.start_kill();
            }
            m.emit(core);
            Err(e)
        }
    }
}

async fn start_inner(core: &Arc<Core>) -> anyhow::Result<()> {
    let m = &core.terracotta;
    let paths = &core.paths;

    // 1. 已有实例（之前没关掉的，或 HMCL / PCL CE 启动的）：直接复用
    if let Some(port) = lock_file_port() {
        if let Ok(meta) = client::meta(&m.http, port).await {
            let owned = read_owned_port(paths) == Some(port);
            let mut g = m.lock();
            g.port = Some(port);
            g.owned = owned;
            g.meta = Some(meta);
            g.phase = Phase::Running;
            g.fails = 0;
            return Ok(());
        }
    }

    // 2. 启动新进程
    let exe = install::exe_path(paths);
    let run_dir = paths.terracotta_dir().join("run");
    tokio::fs::create_dir_all(&run_dir).await?;
    let port_file = run_dir.join(format!("port-{}.json", uuid::Uuid::new_v4().simple()));
    let mode = if cfg!(windows) { "--hmcl2" } else { "--hmcl" };
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg(mode)
        .arg(&port_file)
        .current_dir(install::bundle_dir(paths))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x0800_0000);
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("无法启动联机组件：{}", exe.display()))?;
    {
        let mut g = m.lock();
        g.child = Some(child);
        g.owned = true;
    }

    // 3. 等待端口文件
    let deadline = Instant::now() + Duration::from_secs(20);
    let port: u16 = loop {
        if let Ok(text) = tokio::fs::read_to_string(&port_file).await {
            if let Some(p) = parse_port_file(&text) {
                break p;
            }
        }
        // None：仍在运行；Some(code)：已退出
        let exited: Option<Option<i32>> = {
            let mut g = m.lock();
            let r = match g.child.as_mut() {
                Some(c) => match c.try_wait() {
                    Ok(Some(status)) => Some(status.code()),
                    _ => None,
                },
                None => Some(None),
            };
            r
        };
        if let Some(code) = exited {
            // 附属模式下进程写完端口就会退出，再读一次
            if let Ok(text) = tokio::fs::read_to_string(&port_file).await {
                if let Some(p) = parse_port_file(&text) {
                    break p;
                }
            }
            m.lock().child = None;
            bail!(
                "联机组件启动后立即退出（退出码 {}）。可能被安全软件拦截，或系统缺少必要组件",
                code.map(|c| c.to_string()).unwrap_or_else(|| "未知".into())
            );
        }
        if Instant::now() > deadline {
            bail!("等待联机组件启动超时（20 秒）");
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    let _ = tokio::fs::remove_file(&port_file).await;

    let meta = client::meta(&m.http, port).await.ok();
    // 进程在附属模式下写完端口后退出：说明复用的是其他程序启动的实例
    let still_child = {
        let mut g = m.lock();
        let alive = match g.child.as_mut() {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        };
        if !alive {
            g.child = None;
        }
        alive
    };
    if still_child {
        write_owned_port(paths, port);
    }
    let owned = still_child || read_owned_port(paths) == Some(port);
    let mut g = m.lock();
    g.port = Some(port);
    g.owned = owned;
    g.meta = meta;
    g.phase = Phase::Running;
    g.fails = 0;
    Ok(())
}

/// 停止联机组件（其他程序启动的实例只断开，不关闭）
pub async fn stop(core: &Arc<Core>) -> anyhow::Result<()> {
    let m = &core.terracotta;
    let _op = m.op_lock.lock().await;
    stop_locked(core).await;
    m.emit(core);
    Ok(())
}

/// 安装前调用：只停掉本启动器启动的实例
pub async fn stop_owned(core: &Arc<Core>) {
    let m = &core.terracotta;
    let _op = m.op_lock.lock().await;
    let owned = {
        let g = m.lock();
        g.owned && (g.port.is_some() || g.child.is_some())
    };
    if owned {
        stop_locked(core).await;
        m.emit(core);
    }
}

async fn stop_locked(core: &Arc<Core>) {
    let m = &core.terracotta;
    m.poll_gen.fetch_add(1, Ordering::SeqCst);
    let (port, owned, child) = {
        let mut g = m.lock();
        let port = g.port.take();
        let owned = g.owned;
        let child = g.child.take();
        g.phase = Phase::Stopped;
        g.room = None;
        g.meta = None;
        g.fails = 0;
        g.owned = false;
        (port, owned, child)
    };
    if let Some(port) = port {
        if owned || child.is_some() {
            let _ = client::ide(&m.http, port).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
            client::panic_peaceful(&m.http, port).await;
            clear_owned_port(&core.paths);
        }
    }
    if let Some(mut c) = child {
        let _ = tokio::time::timeout(Duration::from_secs(2), c.wait()).await;
        let _ = c.start_kill();
    }
}

pub async fn restart(core: &Arc<Core>) -> anyhow::Result<()> {
    {
        let m = &core.terracotta;
        let _op = m.op_lock.lock().await;
        stop_locked(core).await;
    }
    start(core).await
}

// ───────────────────────── 房间操作 ─────────────────────────

async fn player_name(core: &Core) -> Option<String> {
    let id = core.settings().selected_account?;
    let store = core.accounts.lock().await;
    let name = store.get(&id).map(|a| a.name.clone());
    name
}

fn require_port(core: &Core) -> anyhow::Result<u16> {
    core.terracotta
        .running_port()
        .ok_or_else(|| anyhow!("联机组件没有运行"))
}

/// 记录新状态（忽略 index 没有增大的旧响应），有变化时推送
fn apply_state(core: &Core, port: u16, room: RoomState) {
    let m = &core.terracotta;
    let changed = {
        let mut g = m.lock();
        if g.port != Some(port) {
            false
        } else {
            g.fails = 0;
            let newer = match &g.room {
                Some(old) => room.index > old.index || (room.index == old.index && room != *old),
                None => true,
            };
            if newer {
                g.room = Some(room);
            }
            newer
        }
    };
    if changed {
        m.emit(core);
    }
}

async fn refresh_now(core: &Arc<Core>) {
    let m = &core.terracotta;
    if let Some(port) = m.running_port() {
        if let Ok(room) = client::state(&m.http, port).await {
            apply_state(core, port, room);
        }
    }
}

/// 设置房主 / 房客前必须处于 waiting（异常、已在房间中时先回到待机）
async fn ensure_waiting(core: &Arc<Core>, port: u16) -> anyhow::Result<()> {
    let m = &core.terracotta;
    let current = client::state(&m.http, port).await?;
    let waiting = current.state == "waiting";
    apply_state(core, port, current);
    if waiting {
        return Ok(());
    }
    client::ide(&m.http, port).await?;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let s = client::state(&m.http, port).await?;
        let ok = s.state == "waiting";
        apply_state(core, port, s);
        if ok {
            return Ok(());
        }
    }
    bail!("联机组件没有回到空闲状态，请稍后再试")
}

/// 作为房主：开始扫描本机的局域网世界，扫描到后自动创建房间
pub async fn host(core: &Arc<Core>) -> anyhow::Result<()> {
    start(core).await?;
    let port = require_port(core)?;
    ensure_waiting(core, port).await?;
    let player = player_name(core).await;
    client::scanning(&core.terracotta.http, port, player.as_deref()).await?;
    refresh_now(core).await;
    Ok(())
}

/// 作为房客：用邀请码加入
pub async fn join(core: &Arc<Core>, code: &str) -> anyhow::Result<()> {
    let code = normalize_room_code(code)
        .ok_or_else(|| anyhow!("邀请码格式不正确，应为 U/XXXX-XXXX-XXXX-XXXX"))?;
    start(core).await?;
    let port = require_port(core)?;
    ensure_waiting(core, port).await?;
    let player = player_name(core).await;
    client::guesting(&core.terracotta.http, port, &code, player.as_deref()).await?;
    refresh_now(core).await;
    Ok(())
}

/// 退出房间 / 关闭房间 / 清除异常：回到待机
pub async fn leave(core: &Arc<Core>) -> anyhow::Result<()> {
    let port = require_port(core)?;
    client::ide(&core.terracotta.http, port).await?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    refresh_now(core).await;
    Ok(())
}

/// 读取联机组件日志（最后 400 行）
pub async fn log_tail(core: &Arc<Core>) -> anyhow::Result<String> {
    let port = require_port(core)?;
    let text = client::log(&core.terracotta.http, port).await?;
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(400);
    Ok(lines[start..].join("\n"))
}

/// 联机页打开 / 关闭
pub async fn watch(core: &Arc<Core>, watching: bool) {
    core.terracotta.set_watching(watching);
    if watching {
        refresh_now(core).await;
    }
}

// ───────────────────────── 后台轮询 ─────────────────────────

fn ensure_poller(core: &Arc<Core>) {
    let gen = core.terracotta.poll_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let core = core.clone();
    tauri::async_runtime::spawn(async move {
        poll_loop(core, gen).await;
    });
}

/// 联机页打开时约 0.7 秒一次；房间进行中（不在 waiting）时约 1 秒一次；其余时间不发请求
async fn poll_loop(core: Arc<Core>, gen: u64) {
    loop {
        let m = &core.terracotta;
        if m.poll_gen.load(Ordering::SeqCst) != gen {
            return;
        }
        let (port, in_session) = {
            let mut g = m.lock();
            // 子进程退出（例如附属模式）不代表服务不可用，是否可用以 HTTP 为准
            let exited = match g.child.as_mut() {
                Some(c) => matches!(c.try_wait(), Ok(Some(_))),
                None => false,
            };
            if exited {
                g.child = None;
            }
            if g.phase != Phase::Running {
                (None, false)
            } else {
                let in_session = g.room.as_ref().map(|r| r.state != "waiting").unwrap_or(false);
                (g.port, in_session)
            }
        };
        let Some(port) = port else {
            return;
        };
        let watching = m.watching.load(Ordering::SeqCst);
        if watching || in_session {
            match client::state(&m.http, port).await {
                Ok(room) => apply_state(&core, port, room),
                Err(e) => {
                    let crashed = {
                        let mut g = m.lock();
                        if g.port != Some(port) {
                            false
                        } else {
                            g.fails += 1;
                            if g.fails >= MAX_FAILS {
                                g.phase = Phase::Crashed(format!("联机组件没有响应，可能已经退出（{e:#}）"));
                                g.port = None;
                                g.room = None;
                                g.meta = None;
                                // 丢弃子进程句柄（kill_on_drop 会结束残留进程）
                                g.child = None;
                                true
                            } else {
                                false
                            }
                        }
                    };
                    if crashed {
                        m.emit(&core);
                        return;
                    }
                }
            }
            let wait = if watching { 700 } else { 1000 };
            tokio::time::sleep(Duration::from_millis(wait)).await;
        } else {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }
}
