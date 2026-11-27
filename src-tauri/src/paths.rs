//! 目录布局（便携模式，类似 PCL：所有东西都放在启动器所在文件夹）。
//!
//! <启动器 exe 所在文件夹>\
//!   pilot-launcher.exe
//!   .minecraft\                    ← 默认游戏根目录（可在设置中修改）
//!     PilotLauncher\               ← 启动器数据目录
//!       settings.json / accounts.json / runtime/<component>/ / authlib/ / cache/
//!     versions/<实例名>/            ← 实例目录，同时作为 --gameDir（版本隔离）
//!       <实例名>.json / <实例名>.jar / natives/ / glass-instance.json
//!       mods/ saves/ resourcepacks/ shaderpacks/ config/
//!     libraries/  assets/          ← 所有实例共享
//!
//! exe 所在文件夹不可写（例如装在 Program Files）时，整体回退到
//! %APPDATA%\dev.pilotlauncher.launcher\（数据）和其中的 .minecraft\（游戏）。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 数据目录在 .minecraft 里的文件夹名
const DATA_DIR_NAME: &str = "PilotLauncher";

/// 改名前（GlassLauncher）的数据目录名与应用标识，用于首次启动时迁移
const OLD_DATA_DIR_NAME: &str = "GlassLauncher";
const OLD_IDENTIFIER: &str = "dev.glasslauncher.launcher";

/// 软件改名后，把旧名字的目录整体改名过来（只在新目录还不存在时）。
/// 改名失败（例如旧目录里有程序正在运行）时，退而复制设置、账户和 authlib-injector。
fn migrate_renamed_dir(old: &Path, new: &Path) {
    if !old.is_dir() || new.exists() {
        return;
    }
    if std::fs::rename(old, new).is_ok() {
        return;
    }
    if std::fs::create_dir_all(new).is_err() {
        return;
    }
    for f in ["settings.json", "accounts.json", "config.local.json"] {
        let from = old.join(f);
        if from.is_file() {
            let _ = std::fs::copy(&from, new.join(f));
        }
    }
    let _ = copy_dir(&old.join("authlib"), &new.join("authlib"));
}

/// exe 同目录下的 .minecraft（便携模式）。目录不可写时返回 None。
fn portable_game_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.join(".minecraft");
        std::fs::create_dir_all(&dir).ok()?;
        // 写入测试，确认真的可写
        let probe = dir.join(".glass-write-test");
        std::fs::write(&probe, b"ok").ok()?;
        let _ = std::fs::remove_file(&probe);
        Some(dir)
    })
    .clone()
}

#[derive(Clone, Debug)]
pub struct Paths {
    /// 当前使用的启动器数据目录
    pub data_dir: PathBuf,
    /// 旧版本使用的数据目录（%APPDATA%\dev.pilotlauncher.launcher）
    pub legacy_data_dir: PathBuf,
}

impl Paths {
    /// 决定数据目录：便携可用时放进 <exe>\.minecraft\PilotLauncher，否则使用 %APPDATA%。
    /// 首次切换到便携目录时，从旧目录迁移设置、账户和 authlib-injector。
    pub fn resolve(app_data_dir: PathBuf) -> std::io::Result<Self> {
        // 软件从 GlassLauncher 改名为 PilotLauncher：先把旧名字的目录迁移过来
        if let Some(parent) = app_data_dir.parent() {
            migrate_renamed_dir(&parent.join(OLD_IDENTIFIER), &app_data_dir);
        }
        let data_dir = match portable_game_dir() {
            Some(g) => {
                let dir = g.join(DATA_DIR_NAME);
                migrate_renamed_dir(&g.join(OLD_DATA_DIR_NAME), &dir);
                dir
            }
            None => app_data_dir.clone(),
        };
        std::fs::create_dir_all(&data_dir)?;
        let paths = Self { data_dir, legacy_data_dir: app_data_dir };
        if paths.data_dir != paths.legacy_data_dir {
            paths.migrate_from_legacy();
        }
        Ok(paths)
    }

    /// 只迁移小文件；Java 运行时（runtime/）体积大，不复制，需要时会自动重新下载
    fn migrate_from_legacy(&self) {
        if self.settings_file().exists() || !self.legacy_data_dir.join("settings.json").exists() {
            return;
        }
        for f in ["settings.json", "accounts.json"] {
            let from = self.legacy_data_dir.join(f);
            if from.is_file() {
                let _ = std::fs::copy(&from, self.data_dir.join(f));
            }
        }
        let _ = copy_dir(&self.legacy_data_dir.join("authlib"), &self.authlib_dir());
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.data_dir.join("accounts.json")
    }
    pub fn runtime_dir(&self) -> PathBuf {
        self.data_dir.join("runtime")
    }
    pub fn authlib_dir(&self) -> PathBuf {
        self.data_dir.join("authlib")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }
    /// 外置登录账户的皮肤 / 披风缓存：<数据目录>/cache/skins/
    pub fn skins_dir(&self) -> PathBuf {
        self.cache_dir().join("skins")
    }
    /// 联机组件（Terracotta | 陶瓦联机）：<数据目录>/terracotta/<版本>/
    pub fn terracotta_dir(&self) -> PathBuf {
        self.data_dir.join("terracotta")
    }
    /// 默认游戏目录：优先 exe 同目录的 .minecraft，不可写时回退到 %APPDATA% 下
    pub fn default_game_dir(&self) -> PathBuf {
        portable_game_dir().unwrap_or_else(|| self.legacy_game_dir())
    }
    /// 旧版本（第一/二阶段）使用的默认游戏目录
    pub fn legacy_game_dir(&self) -> PathBuf {
        self.legacy_data_dir.join(".minecraft")
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if !target.exists() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct GameDirs {
    pub root: PathBuf,
}

impl GameDirs {
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    /// 实例目录，也就是启动时的 --gameDir
    pub fn instance_dir(&self, name: &str) -> PathBuf {
        self.versions().join(name)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.jar"))
    }
    pub fn natives_dir(&self, name: &str) -> PathBuf {
        self.instance_dir(name).join("natives")
    }
}
