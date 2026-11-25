//! 启动流程：文件检查 → Java → 账户 → natives/资源 → 拼参数 → 启动进程 → 日志采集 → 崩溃分析。

pub mod args;
pub mod crash;
pub mod natives;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context};
use serde::Serialize;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;

use self::args::Vars;
use self::crash::CrashInfo;
use crate::auth::store::{Account, AccountKind};
use crate::auth::{authlib, yggdrasil};
use crate::instance::{self, InstanceMeta};
use crate::java::{detect, runtime, JavaInfo};
use crate::minecraft::assets::{ensure_index, prepare_legacy_assets};
use crate::minecraft::install;
use crate::minecraft::library::{classpath, resolve_libraries};
use crate::minecraft::rules::Features;
use crate::minecraft::version::{load_resolved, VersionJson};
use crate::settings::{MemoryMode, Settings};
use crate::state::{Core, LogBuffer};
use crate::system;
use crate::util::split_args;

pub const STATE_EVENT: &str = "game-state";
pub const LOG_EVENT: &str = "game-log";
const LOG_CAPACITY: usize = 5000;
const LAUNCHER_NAME: &str = "PilotLauncher";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameStateEvent {
    pub instance: String,
    /// preparing / running / exited / error
    pub state: String,
    pub message: Option<String>,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub killed: bool,
    pub crash: Option<CrashInfo>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameLogEvent {
    pub instance: String,
    pub lines: Vec<String>,
}

fn emit_state(core: &Core, ev: GameStateEvent) {
    let _ = core.app.emit(STATE_EVENT, ev);
}

fn preparing(core: &Core, instance: &str, message: &str) {
    emit_state(
        core,
        GameStateEvent {
            instance: instance.to_string(),
            state: "preparing".into(),
            message: Some(message.to_string()),
            pid: None,
            exit_code: None,
            killed: false,
            crash: None,
        },
    );
}

struct AgentArgs {
    jar: PathBuf,
    root: String,
    metadata_b64: String,
}

struct AuthArgs {
    name: String,
    uuid: String,
    token: String,
    user_type: &'static str,
    agent: Option<AgentArgs>,
}

/// 启动入口：返回时游戏进程已经启动（或失败）
pub async fn launch(core: Arc<Core>, name: String) -> anyhow::Result<()> {
    {
        if core.is_running(&name) {
            bail!("实例 {name} 已在运行或正在启动");
        }
        core.launching
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.clone());
    }
    let result = launch_inner(&core, &name).await;
    core.launching
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&name);
    if let Err(e) = &result {
        emit_state(
            &core,
            GameStateEvent {
                instance: name.clone(),
                state: "error".into(),
                message: Some(format!("{:#}", e)),
                pid: None,
                exit_code: None,
                killed: false,
                crash: None,
            },
        );
    }
    result
}

async fn resolve_java(
    core: &Core,
    meta: &InstanceMeta,
    settings: &Settings,
    version: &VersionJson,
) -> anyhow::Result<JavaInfo> {
    if let Some(p) = meta.java_path.clone().or_else(|| settings.java_path.clone()) {
        return detect::inspect(&p, "自定义").ok_or_else(|| anyhow!("找不到指定的 Java：{p}"));
    }
    let (component, major) = version.java_requirement();
    if let Some(info) = runtime::find_installed(&core.paths, component.as_deref(), major) {
        return Ok(info);
    }
    let task = core.tasks.create(format!("下载 Java {major} 运行时"), "java");
    let r = runtime::ensure_runtime(core, &task, component.as_deref(), major, (1, 1)).await;
    let unit: anyhow::Result<()> = match &r {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("{:#}", e)),
    };
    task.finish(&unit);
    r
}

async fn prepare_account(core: &Core, mut account: Account) -> anyhow::Result<AuthArgs> {
    match account.kind {
        AccountKind::Offline => Ok(AuthArgs {
            name: account.name,
            uuid: account.uuid,
            token: account.access_token,
            user_type: "msa",
            agent: None,
        }),
        AccountKind::Yggdrasil => {
            let root = account
                .server_url
                .clone()
                .ok_or_else(|| anyhow!("外置账户缺少认证服务器地址"))?;
            let valid = yggdrasil::validate(&core.http, &root, &account.access_token, &account.client_token)
                .await
                .unwrap_or(false);
            if !valid {
                let r = yggdrasil::refresh(&core.http, &root, &account.access_token, &account.client_token, None)
                    .await
                    .context("登录已过期，请在账户管理中重新登录")?;
                account.access_token = r.access_token;
                if let Some(ct) = r.client_token {
                    account.client_token = ct;
                }
                if let Some(p) = r.selected_profile {
                    account.name = p.name;
                    account.uuid = p.id;
                }
            }
            let jar = authlib::ensure_authlib(core).await?;
            // 每次启动时刷新元数据；失败时使用登录时缓存的版本
            if let Ok(info) = yggdrasil::resolve_server(&core.http, &root).await {
                account.metadata_b64 = Some(info.metadata_b64);
            }
            core.accounts.lock().await.update(account.clone())?;
            Ok(AuthArgs {
                name: account.name,
                uuid: account.uuid,
                token: account.access_token,
                user_type: "mojang",
                agent: Some(AgentArgs {
                    jar,
                    root,
                    metadata_b64: account.metadata_b64.unwrap_or_default(),
                }),
            })
        }
    }
}

async fn launch_inner(core: &Arc<Core>, name: &str) -> anyhow::Result<()> {
    let dirs = core.game_dirs();
    let settings = core.settings();

    // 账户
    preparing(core, name, "检查账户");
    let account = {
        let store = core.accounts.lock().await;
        let id = settings
            .selected_account
            .clone()
            .ok_or_else(|| anyhow!("请先在首页添加并选择一个账户"))?;
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("所选账户不存在，请重新选择"))?
    };

    let meta = instance::load_meta(&dirs, name)?;
    let version = load_resolved(&dirs, name)?;
    let main_class = version
        .main_class
        .clone()
        .ok_or_else(|| anyhow!("版本 JSON 缺少 mainClass"))?;

    // 1. 文件检查
    preparing(core, name, "检查游戏文件");
    let missing = install::missing_files_quick(core, name).await?;
    if !missing.is_empty() {
        let task = core.tasks.create(format!("补全 {name} 的游戏文件"), "launch");
        let r = install::download_items(core, &task, missing).await;
        task.finish(&r);
        r?;
    }

    // 2. Java
    preparing(core, name, "准备 Java");
    let java = resolve_java(core, &meta, &settings, &version).await?;
    let (_, required_major) = version.java_requirement();
    if java.major != 0 && java.major < required_major {
        bail!(
            "该版本需要 Java {required_major}，当前选择的是 Java {}（{}）。请修改 Java 设置。",
            java.major,
            java.path
        );
    }

    // 3. 账户验证 / authlib-injector
    preparing(core, name, "验证账户");
    let auth = prepare_account(core, account).await?;

    // 4. natives / 资源
    preparing(core, name, "解压本地库");
    let width = meta.window_width.unwrap_or(settings.window_width);
    let height = meta.window_height.unwrap_or(settings.window_height);
    let features = Features { has_custom_resolution: width > 0 && height > 0 };
    let libs = resolve_libraries(&version, &dirs.libraries(), &features, &|u: &str| vec![u.to_string()]);
    let natives_dir = dirs.natives_dir(name);
    {
        let libs2 = libs.clone();
        let nd = natives_dir.clone();
        tokio::task::spawn_blocking(move || natives::extract_natives(&libs2, &nd)).await??;
    }
    let game_dir = dirs.instance_dir(name);
    for sub in instance::SUBDIRS {
        let _ = std::fs::create_dir_all(game_dir.join(sub));
    }
    let assets_root = dirs.assets();
    let (assets_index_name, game_assets) = match &version.asset_index {
        Some(idx) => {
            let index = ensure_index(core, idx, &assets_root).await?;
            let legacy = prepare_legacy_assets(index, assets_root.clone(), game_dir.clone()).await?;
            (idx.id.clone(), legacy.unwrap_or_else(|| assets_root.clone()))
        }
        None => (
            version.assets.clone().unwrap_or_else(|| "legacy".into()),
            assets_root.clone(),
        ),
    };

    // 5. 参数
    let client_jar = dirs.version_jar(&version.jar_id());
    if !client_jar.is_file() {
        bail!("缺少客户端文件 {}，请修复实例", client_jar.display());
    }
    let cp = classpath(&libs, &client_jar);
    if let Some(missing) = cp.iter().find(|p| !p.is_file()) {
        bail!("缺少依赖库 {}，请修复实例", missing.display());
    }
    let cp_str = cp
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(";");

    let mut vars = Vars::new();
    vars.insert("auth_player_name", auth.name.clone());
    vars.insert("version_name", name.to_string());
    vars.insert("game_directory", game_dir.display().to_string());
    vars.insert("assets_root", assets_root.display().to_string());
    vars.insert("assets_index_name", assets_index_name);
    vars.insert("game_assets", game_assets.display().to_string());
    vars.insert("auth_uuid", auth.uuid.clone());
    vars.insert("auth_access_token", auth.token.clone());
    vars.insert("auth_session", auth.token.clone());
    vars.insert("auth_xuid", "0".into());
    vars.insert("clientid", "0".into());
    vars.insert("user_type", auth.user_type.to_string());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", LAUNCHER_NAME.into());
    vars.insert("resolution_width", width.to_string());
    vars.insert("resolution_height", height.to_string());
    vars.insert("natives_directory", natives_dir.display().to_string());
    vars.insert("launcher_name", LAUNCHER_NAME.into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", cp_str);
    vars.insert("classpath_separator", ";".into());
    vars.insert("library_directory", dirs.libraries().display().to_string());
    vars.insert("primary_jar", client_jar.display().to_string());

    let memory = meta.memory_mb.unwrap_or(match settings.memory_mode {
        MemoryMode::Auto => system::auto_memory_mb(),
        MemoryMode::Manual => settings.memory_mb,
    });
    let mut all_args: Vec<String> = vec![
        format!("-Xmx{memory}m"),
        format!("-Xmn{}m", (memory * 15 / 100).max(128)),
        "-XX:+UseG1GC".into(),
        "-XX:-UseAdaptiveSizePolicy".into(),
        "-XX:-OmitStackTraceInFastThrow".into(),
        "-Dfile.encoding=UTF-8".into(),
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
        "-Dsun.stdout.encoding=UTF-8".into(),
        "-Dsun.stderr.encoding=UTF-8".into(),
        "-Dlog4j2.formatMsgNoLookups=true".into(),
        "-Dfml.ignoreInvalidMinecraftCertificates=true".into(),
        "-Dfml.ignorePatchDiscrepancies=true".into(),
    ];
    all_args.extend(split_args(&settings.jvm_args));
    all_args.extend(split_args(&meta.jvm_args));
    if let Some(agent) = &auth.agent {
        all_args.push(format!("-javaagent:{}={}", agent.jar.display(), agent.root));
        all_args.push("-Dauthlibinjector.side=client".into());
        if !agent.metadata_b64.is_empty() {
            all_args.push(format!("-Dauthlibinjector.yggdrasil.prebuilt={}", agent.metadata_b64));
        }
    }
    all_args.extend(args::jvm_args(&version, &features, &vars));
    all_args.push(main_class);
    let (game_args, legacy) = args::game_args(&version, &features, &vars);
    all_args.extend(game_args);
    if legacy && features.has_custom_resolution {
        all_args.push("--width".into());
        all_args.push(width.to_string());
        all_args.push("--height".into());
        all_args.push(height.to_string());
    }
    all_args.extend(split_args(&settings.game_args));
    all_args.extend(split_args(&meta.game_args));

    // Windows 命令行上限 32767 字符；Java 9+ 支持 @argfile
    let total_len: usize = all_args.iter().map(|a| a.len() + 3).sum();
    let final_args = if total_len > 30000 && java.major >= 9 {
        let argfile = game_dir.join(".glass-launch-args.txt");
        std::fs::write(&argfile, args::argfile_content(&all_args))?;
        vec![format!("@{}", argfile.display())]
    } else {
        all_args.clone()
    };

    // 6. 启动进程
    preparing(core, name, "启动游戏进程");
    let log: LogBuffer = Arc::new(Mutex::new(VecDeque::new()));
    core.logs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(name.to_string(), log.clone());
    let mask = |s: &str| -> String {
        if auth.token.len() >= 8 {
            s.replace(&auth.token, "******")
        } else {
            s.to_string()
        }
    };
    {
        let mut l = log.lock().unwrap_or_else(|e| e.into_inner());
        l.push_back(format!("[PilotLauncher] Java：{}（{}）", java.path, java.version));
        l.push_back(format!("[PilotLauncher] 游戏目录：{}", game_dir.display()));
        l.push_back(format!("[PilotLauncher] 启动参数：{}", mask(all_args.join(" ").as_str())));
    }

    let mut cmd = tokio::process::Command::new(&java.path);
    cmd.args(&final_args)
        .current_dir(&game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x0800_0000);
    }
    let started = SystemTime::now();
    let mut child = cmd
        .spawn()
        .with_context(|| format!("无法启动 Java 进程：{}", java.path))?;
    let pid = child.id();
    let _ = instance::touch_last_played(&dirs, name);

    // 日志采集
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    if let Some(out) = child.stdout.take() {
        spawn_reader(out, tx.clone());
    }
    if let Some(err) = child.stderr.take() {
        spawn_reader(err, tx.clone());
    }
    drop(tx);
    let token = auth.token.clone();
    spawn_log_pump(core.clone(), name.to_string(), rx, log.clone(), token);

    // 结束信号
    let (kill_tx, mut kill_rx) = tokio::sync::oneshot::channel::<()>();
    core.games
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(name.to_string(), kill_tx);

    emit_state(
        core,
        GameStateEvent {
            instance: name.to_string(),
            state: "running".into(),
            message: Some("游戏已启动".into()),
            pid,
            exit_code: None,
            killed: false,
            crash: None,
        },
    );

    let minimize = settings.launcher_visibility == "minimize";
    if minimize {
        if let Some(w) = core.main_window() {
            let _ = w.minimize();
        }
    }

    let core2 = core.clone();
    let name2 = name.to_string();
    tauri::async_runtime::spawn(async move {
        let mut killed = false;
        let status = tokio::select! {
            s = child.wait() => s.ok(),
            Ok(()) = &mut kill_rx => {
                killed = true;
                let _ = child.kill().await;
                child.wait().await.ok()
            }
        };
        core2
            .games
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&name2);
        // 等待日志读取完毕
        tokio::time::sleep(Duration::from_millis(300)).await;
        let code = status.and_then(|s| s.code());
        let crash = if !killed && code != Some(0) {
            let lines: Vec<String> = log.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
            Some(crash::analyze(code, &lines, &game_dir, started))
        } else {
            None
        };
        if minimize {
            if let Some(w) = core2.main_window() {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }
        emit_state(
            &core2,
            GameStateEvent {
                instance: name2,
                state: "exited".into(),
                message: Some(match code {
                    Some(c) => format!("游戏已退出（退出码 {c}）"),
                    None => "游戏已退出".into(),
                }),
                pid,
                exit_code: code,
                killed,
                crash,
            },
        );
    });
    Ok(())
}

fn spawn_reader<R: AsyncRead + Unpin + Send + 'static>(reader: R, tx: mpsc::UnboundedSender<String>) {
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf)
                        .trim_end_matches(|c: char| c == '\r' || c == '\n')
                        .to_string();
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// 批量推送日志，避免每行一个事件拖慢前端
fn spawn_log_pump(
    core: Arc<Core>,
    instance: String,
    mut rx: mpsc::UnboundedReceiver<String>,
    log: LogBuffer,
    token: String,
) {
    tauri::async_runtime::spawn(async move {
        let mut batch: Vec<String> = Vec::new();
        let flush = |batch: &mut Vec<String>| {
            if batch.is_empty() {
                return;
            }
            let lines = std::mem::take(batch);
            let _ = core.app.emit(
                LOG_EVENT,
                GameLogEvent { instance: instance.clone(), lines },
            );
        };
        loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(mut l) => {
                        if token.len() >= 8 && l.contains(&token) {
                            l = l.replace(&token, "******");
                        }
                        {
                            let mut buf = log.lock().unwrap_or_else(|e| e.into_inner());
                            if buf.len() >= LOG_CAPACITY {
                                buf.pop_front();
                            }
                            buf.push_back(l.clone());
                        }
                        batch.push(l);
                        if batch.len() >= 200 {
                            flush(&mut batch);
                        }
                    }
                    None => {
                        flush(&mut batch);
                        break;
                    }
                },
                _ = tokio::time::sleep(Duration::from_millis(120)), if !batch.is_empty() => {
                    flush(&mut batch);
                }
            }
        }
    });
}

/// 结束游戏进程
pub fn kill(core: &Core, name: &str) -> anyhow::Result<()> {
    let sender = core
        .games
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(name);
    match sender {
        Some(tx) => {
            let _ = tx.send(());
            Ok(())
        }
        None => bail!("实例 {name} 没有在运行"),
    }
}

pub fn get_log(core: &Core, name: &str) -> Vec<String> {
    core.logs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(name)
        .map(|l| l.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect())
        .unwrap_or_default()
}
