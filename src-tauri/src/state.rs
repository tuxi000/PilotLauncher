//! 全局状态。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::anyhow;
use tauri::{AppHandle, Manager};

use crate::auth::store::AccountStore;
use crate::download::{self, source};
use crate::minecraft::manifest::VersionManifest;
use crate::paths::{GameDirs, Paths};
use crate::settings::{LocalConfig, Settings};
use crate::tasks::TaskManager;

pub type LogBuffer = Arc<Mutex<VecDeque<String>>>;

pub struct Core {
    pub app: AppHandle,
    pub paths: Paths,
    pub http: reqwest::Client,
    settings: RwLock<Settings>,
    pub tasks: TaskManager,
    pub accounts: tokio::sync::Mutex<AccountStore>,
    pub manifest_cache: tokio::sync::Mutex<Option<(Instant, VersionManifest)>>,
    pub local_config: LocalConfig,
    /// 正在运行的游戏：实例名 → 结束信号
    pub games: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
    /// 正在准备启动的实例
    pub launching: Mutex<HashSet<String>>,
    /// 每个实例最近一次运行的日志
    pub logs: Mutex<HashMap<String, LogBuffer>>,
    /// 联机组件（Terracotta | 陶瓦联机）
    pub terracotta: crate::terracotta::Manager,
}

impl Core {
    pub fn init(app: AppHandle) -> anyhow::Result<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| anyhow!("无法获取应用数据目录：{e}"))?;
        let paths = Paths::resolve(app_data_dir).map_err(|e| anyhow!("无法创建数据目录：{e}"))?;
        let settings = Settings::load(&paths.settings_file());
        let accounts = AccountStore::load(paths.accounts_file());
        let local_config = LocalConfig::load(&paths);
        Ok(Self {
            tasks: TaskManager::new(app.clone()),
            app,
            http: download::build_client()?,
            settings: RwLock::new(settings),
            accounts: tokio::sync::Mutex::new(accounts),
            manifest_cache: tokio::sync::Mutex::new(None),
            local_config,
            games: Mutex::new(HashMap::new()),
            launching: Mutex::new(HashSet::new()),
            logs: Mutex::new(HashMap::new()),
            terracotta: crate::terracotta::Manager::new()?,
            paths,
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn save_settings(&self, mut s: Settings) -> anyhow::Result<Settings> {
        s.normalize();
        s.save(&self.paths.settings_file())?;
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = s.clone();
        Ok(s)
    }

    /// 修改设置中的某些字段并保存
    pub fn update_settings<F: FnOnce(&mut Settings)>(&self, f: F) -> anyhow::Result<Settings> {
        let mut s = self.settings();
        f(&mut s);
        self.save_settings(s)
    }

    pub fn game_dirs(&self) -> GameDirs {
        let root = self
            .settings()
            .game_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.paths.default_game_dir());
        GameDirs { root }
    }

    /// 按当前下载源生成候选地址
    pub fn sources(&self, url: &str) -> Vec<String> {
        source::candidates(self.settings().download_source, url)
    }

    pub fn is_running(&self, instance: &str) -> bool {
        self.games
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(instance)
            || self
                .launching
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(instance)
    }

    pub fn running_instances(&self) -> Vec<String> {
        self.games
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect()
    }

    pub fn main_window(&self) -> Option<tauri::WebviewWindow> {
        self.app.get_webview_window("main")
    }
}
