//! 本机 Java 检测。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::parse_major;
use crate::paths::Paths;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    /// javaw.exe 的完整路径
    pub path: String,
    pub version: String,
    /// 0 表示无法识别
    pub major: u32,
    /// 来源说明：官方运行时 / JAVA_HOME / PATH / 已安装 等
    pub source: String,
}

/// 读取 Java 安装目录下 release 文件中的 JAVA_VERSION
pub fn read_release_version(home: &Path) -> Option<String> {
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("JAVA_VERSION="))
        .map(|v| v.trim().trim_matches('"').to_string())
}

/// 把用户输入的路径规范化为 javaw.exe：
/// 可以是 Java 安装目录、bin 目录、java.exe 或 javaw.exe。
pub fn normalize_java_path(input: &str) -> Option<PathBuf> {
    let p = PathBuf::from(input.trim().trim_matches('"'));
    let candidates = if p.is_dir() {
        vec![
            p.join("bin").join("javaw.exe"),
            p.join("javaw.exe"),
            p.join("bin").join("java.exe"),
            p.join("java.exe"),
        ]
    } else {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if name == "java.exe" {
            vec![p.with_file_name("javaw.exe"), p.clone()]
        } else {
            vec![p.clone()]
        }
    };
    candidates.into_iter().find(|c| c.is_file())
}

pub fn inspect(input: &str, source: &str) -> Option<JavaInfo> {
    let exe = normalize_java_path(input)?;
    let home = exe.parent().and_then(|bin| bin.parent()).map(|h| h.to_path_buf());
    let version = home
        .as_deref()
        .and_then(read_release_version)
        .unwrap_or_else(|| "未知".to_string());
    let major = parse_major(&version).unwrap_or(0);
    Some(JavaInfo {
        path: exe.display().to_string(),
        version,
        major,
        source: source.to_string(),
    })
}

fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// 扫描常见位置的 Java
pub fn scan(paths: &Paths) -> Vec<JavaInfo> {
    let mut found: Vec<(PathBuf, &str)> = Vec::new();

    // 1. 本启动器下载的官方运行时
    for dir in child_dirs(&paths.runtime_dir()) {
        found.push((dir, "PilotLauncher 官方运行时"));
    }
    // 2. JAVA_HOME
    if let Ok(home) = std::env::var("JAVA_HOME") {
        found.push((PathBuf::from(home), "JAVA_HOME"));
    }
    // 3. PATH
    if let Ok(path) = std::env::var("PATH") {
        for entry in std::env::split_paths(&path) {
            if entry.join("java.exe").is_file() || entry.join("javaw.exe").is_file() {
                found.push((entry, "PATH"));
            }
        }
    }
    // 4. 常见安装目录
    let vendors = [
        "Java",
        "Eclipse Adoptium",
        "Eclipse Foundation",
        "Microsoft",
        "Zulu",
        "BellSoft",
        "Amazon Corretto",
        "Semeru",
        "OpenJDK",
        "Liberica",
    ];
    for env in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(pf) = std::env::var(env) {
            for v in vendors {
                for dir in child_dirs(&Path::new(&pf).join(v)) {
                    found.push((dir, "已安装"));
                }
            }
        }
    }
    // 5. 官方启动器自带的运行时
    let mut official_roots = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        official_roots.push(Path::new(&appdata).join(".minecraft").join("runtime"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        official_roots.push(
            Path::new(&local)
                .join("Packages")
                .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                .join("LocalCache")
                .join("Local")
                .join("runtime"),
        );
    }
    for root in official_roots {
        // runtime/<component>/windows-x64/<component>/
        for comp in child_dirs(&root) {
            for plat in child_dirs(&comp) {
                for home in child_dirs(&plat) {
                    found.push((home, "官方启动器"));
                }
            }
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (dir, source) in found {
        if let Some(info) = inspect(&dir.to_string_lossy(), source) {
            if seen.insert(info.path.to_lowercase()) {
                out.push(info);
            }
        }
    }
    out.sort_by(|a, b| b.major.cmp(&a.major));
    out
}
