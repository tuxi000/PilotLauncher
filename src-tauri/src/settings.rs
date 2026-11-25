//! 启动器设置（settings.json）与本地私密配置（config.local.json）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::util::{read_json, write_json_atomic};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSource {
    /// 只用 Mojang 官方源
    Official,
    /// BMCLAPI 镜像优先，镜像失败时再回退官方
    Bmclapi,
    /// 官方优先，失败自动切换到 BMCLAPI
    #[default]
    OfficialFirst,
}

/// 社区资源（Modrinth / CurseForge）的下载源策略：镜像为 MCIM（mod.mcimirror.top）
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommunitySource {
    /// 官方优先，失败自动切换到 MCIM
    #[default]
    OfficialFirst,
    /// MCIM 镜像优先，失败回退官方
    MirrorFirst,
    /// 只用官方 API 与 CDN
    Official,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub download_source: DownloadSource,
    /// 社区资源（Mod / 整合包 / 资源包 …）的源策略
    pub community_source: CommunitySource,
    /// 并发下载数（1~128）
    pub concurrency: usize,
    /// 每个文件的重试次数
    pub retries: u32,
    pub memory_mode: MemoryMode,
    /// 手动模式下的最大内存（MB）
    pub memory_mb: u32,
    /// 全局 Java 路径覆盖（为空则自动下载/选择官方运行时）
    pub java_path: Option<String>,
    /// system / light / dark
    pub theme: String,
    /// mica / acrylic / tabbed / none
    pub window_effect: String,
    /// 是否启用 SVG 位移折射（较耗性能）
    pub glass_refraction: bool,
    /// 自定义游戏根目录（为空使用默认 .minecraft）
    pub game_dir: Option<String>,
    pub selected_instance: Option<String>,
    pub selected_account: Option<String>,
    /// 全局附加 JVM 参数
    pub jvm_args: String,
    /// 全局附加游戏参数
    pub game_args: String,
    /// 游戏窗口大小，0 表示默认
    pub window_width: u32,
    pub window_height: u32,
    /// 自定义首页背景图（本地图片路径）
    pub background_image: Option<String>,
    /// 内置背景（未设置自定义背景图时显示）的配色：blue / mint / coral
    #[serde(default = "default_background_palette")]
    pub background_palette: String,
    /// 内置背景是否缓慢流动（系统开启“减少动画”时前端会自动停止）
    #[serde(default = "default_true")]
    pub background_animation: bool,
    /// 启动游戏后启动器的行为：keep / minimize
    pub launcher_visibility: String,
}

/// 背景配色的合法取值
pub const BACKGROUND_PALETTES: &[&str] = &["blue", "mint", "coral"];

fn default_background_palette() -> String {
    "blue".into()
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_source: DownloadSource::OfficialFirst,
            community_source: CommunitySource::OfficialFirst,
            concurrency: 32,
            retries: 3,
            memory_mode: MemoryMode::Auto,
            memory_mb: 4096,
            java_path: None,
            theme: "system".into(),
            window_effect: "mica".into(),
            glass_refraction: true,
            game_dir: None,
            selected_instance: None,
            selected_account: None,
            jvm_args: String::new(),
            game_args: String::new(),
            window_width: 0,
            window_height: 0,
            background_image: None,
            background_palette: default_background_palette(),
            background_animation: true,
            launcher_visibility: "keep".into(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut s: Settings = read_json(path).unwrap_or_default();
        s.normalize();
        s
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        write_json_atomic(path, self)
    }

    /// 把非法取值修正到合理范围
    pub fn normalize(&mut self) {
        self.concurrency = self.concurrency.clamp(1, 128);
        self.retries = self.retries.min(10);
        self.memory_mb = self.memory_mb.clamp(512, 65536);
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            self.theme = "system".into();
        }
        if !["mica", "acrylic", "tabbed", "none"].contains(&self.window_effect.as_str()) {
            self.window_effect = "mica".into();
        }
        if !["keep", "minimize"].contains(&self.launcher_visibility.as_str()) {
            self.launcher_visibility = "keep".into();
        }
        if !BACKGROUND_PALETTES.contains(&self.background_palette.as_str()) {
            self.background_palette = default_background_palette();
        }
        for opt in [&mut self.java_path, &mut self.game_dir, &mut self.background_image] {
            if opt.as_deref().map(|s| s.trim().is_empty()).unwrap_or(false) {
                *opt = None;
            }
        }
    }
}

/// 本地私密配置，不进入版本控制（见 .gitignore）。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalConfig {
    /// CurseForge API Key
    pub curseforge_api_key: Option<String>,
    /// 读取到配置的文件路径（仅用于展示）
    #[serde(skip_deserializing)]
    pub loaded_from: Option<String>,
}

impl LocalConfig {
    /// 依次在以下位置查找 config.local.json：
    /// 1. 可执行文件所在目录  2. 启动器数据目录  3. 开发模式下的项目根目录
    pub fn load(paths: &Paths) -> Self {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("config.local.json"));
            }
        }
        candidates.push(paths.data_dir.join("config.local.json"));
        if cfg!(debug_assertions) {
            candidates.push(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("config.local.json"),
            );
        }
        let mut found: Option<LocalConfig> = None;
        for path in candidates {
            if path.is_file() {
                if let Ok(mut cfg) = read_json::<LocalConfig>(&path) {
                    if cfg
                        .curseforge_api_key
                        .as_deref()
                        .map(|k| k.trim().is_empty() || k.contains("在此填写"))
                        .unwrap_or(false)
                    {
                        cfg.curseforge_api_key = None;
                    }
                    cfg.loaded_from = Some(path.display().to_string());
                    found = Some(cfg);
                    break;
                }
            }
        }
        let mut cfg = found.unwrap_or_default();
        // 本地文件里没有 Key 时，使用构建时内置的 Key（发布版由 GitHub Actions 注入，源码里不含真实 Key）
        if cfg.curseforge_api_key.is_none() {
            if let Some(key) = option_env!("PILOT_CURSEFORGE_API_KEY") {
                let key = key.trim();
                if !key.is_empty() {
                    cfg.curseforge_api_key = Some(key.to_string());
                    cfg.loaded_from = Some("构建时内置".to_string());
                }
            }
        }
        cfg
    }
}
