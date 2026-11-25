//! 崩溃分析：根据退出码、日志关键字和崩溃报告给出中文提示。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrashInfo {
    pub title: String,
    pub suggestions: Vec<String>,
    pub report_path: Option<String>,
    pub report_excerpt: Option<String>,
    pub log_tail: Vec<String>,
}

struct Pattern {
    needles: &'static [&'static str],
    title: &'static str,
    suggestion: &'static str,
}

const PATTERNS: &[Pattern] = &[
    Pattern {
        needles: &["java.lang.OutOfMemoryError", "Out of memory"],
        title: "内存不足",
        suggestion: "在设置或实例设置中调高最大内存；如果已经很高，请关闭其他占用内存的程序。",
    },
    Pattern {
        needles: &["Could not reserve enough space", "Invalid maximum heap size", "Initial heap size set to a larger value"],
        title: "内存分配失败",
        suggestion: "分配的内存超过了可用范围，请调低最大内存，并确认使用的是 64 位 Java。",
    },
    Pattern {
        needles: &["UnsupportedClassVersionError", "has been compiled by a more recent version of the Java Runtime"],
        title: "Java 版本过低",
        suggestion: "该版本需要更高版本的 Java，请清除 Java 路径覆盖，让启动器自动选择官方运行时。",
    },
    Pattern {
        needles: &["Unrecognized VM option", "Unrecognized option", "Could not create the Java Virtual Machine"],
        title: "JVM 参数错误",
        suggestion: "附加 JVM 参数中包含当前 Java 不支持的选项，请检查设置中的 JVM 参数。",
    },
    Pattern {
        needles: &["Pixel format not accelerated", "GLFW error 65542", "WGL: The driver does not appear to support OpenGL", "OpenGL 1.4", "No OpenGL context"],
        title: "显卡驱动不支持 OpenGL",
        suggestion: "请到显卡厂商官网更新驱动；笔记本请确认 Java 使用的是独立显卡。",
    },
    Pattern {
        needles: &["java.lang.ClassNotFoundException", "java.lang.NoClassDefFoundError", "Could not find or load main class", "ZipException", "invalid LOC header"],
        title: "游戏文件缺失或损坏",
        suggestion: "在实例页面对该实例执行“修复文件”。",
    },
    Pattern {
        needles: &["UnsatisfiedLinkError", "Failed to locate library: lwjgl"],
        title: "本地库（natives）加载失败",
        suggestion: "请执行“修复文件”；如果路径中包含特殊字符，请在设置中把游戏目录换到纯英文路径。",
    },
    Pattern {
        needles: &["Failed to login: Invalid session", "Invalid session (Try restarting your game", "AuthenticationUnavailableException"],
        title: "登录状态无效",
        suggestion: "请在账户管理中重新登录外置账户。",
    },
    Pattern {
        needles: &["Mixin apply failed", "ModResolutionException", "Missing or unsupported mandatory dependencies", "DuplicateModsFoundException"],
        title: "Mod 加载失败",
        suggestion: "存在缺失依赖、版本不匹配或重复的 Mod，请检查实例 mods 文件夹。",
    },
];

/// 查找本次启动之后生成的最新崩溃报告
fn latest_report(game_dir: &Path, since: SystemTime) -> Option<PathBuf> {
    let mut best: Option<(SystemTime, PathBuf)> = None;
    let mut consider = |p: PathBuf| {
        if let Ok(m) = std::fs::metadata(&p).and_then(|m| m.modified()) {
            if m >= since && best.as_ref().map(|b| m > b.0).unwrap_or(true) {
                best = Some((m, p));
            }
        }
    };
    if let Ok(rd) = std::fs::read_dir(game_dir.join("crash-reports")) {
        for e in rd.filter_map(|e| e.ok()) {
            consider(e.path());
        }
    }
    // JVM 自身崩溃时生成 hs_err_pid*.log
    if let Ok(rd) = std::fs::read_dir(game_dir) {
        for e in rd.filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().starts_with("hs_err_pid") {
                consider(e.path());
            }
        }
    }
    best.map(|b| b.1)
}

pub fn analyze(exit_code: Option<i32>, log: &[String], game_dir: &Path, started: SystemTime) -> CrashInfo {
    let report_path = latest_report(game_dir, started);
    let report_text = report_path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| String::from_utf8_lossy(&b).to_string());

    let mut haystack = log.join("\n");
    if let Some(t) = &report_text {
        haystack.push('\n');
        haystack.push_str(t);
    }

    let mut title = None;
    let mut suggestions = Vec::new();
    for p in PATTERNS {
        if p.needles.iter().any(|n| haystack.contains(n)) {
            if title.is_none() {
                title = Some(p.title.to_string());
            }
            suggestions.push(format!("{}：{}", p.title, p.suggestion));
        }
    }

    // 崩溃报告中的 Description 行
    let description = report_text.as_ref().and_then(|t| {
        t.lines()
            .find_map(|l| l.trim().strip_prefix("Description:"))
            .map(|d| d.trim().to_string())
    });
    let title = title.unwrap_or_else(|| match &description {
        Some(d) => format!("游戏崩溃：{d}"),
        None => match exit_code {
            Some(c) => format!("游戏异常退出（退出码 {c}）"),
            None => "游戏异常退出".to_string(),
        },
    });
    if suggestions.is_empty() {
        suggestions.push("查看下方日志与崩溃报告定位原因；也可以尝试“修复文件”或更换 Java。".into());
    }

    let excerpt = report_text.map(|t| t.lines().take(40).collect::<Vec<_>>().join("\n"));
    let tail_start = log.len().saturating_sub(80);
    CrashInfo {
        title,
        suggestions,
        report_path: report_path.map(|p| p.display().to_string()),
        report_excerpt: excerpt,
        log_tail: log[tail_start..].to_vec(),
    }
}
