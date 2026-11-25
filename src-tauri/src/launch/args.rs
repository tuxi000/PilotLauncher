//! 启动参数构建：变量替换、rules 过滤、旧版 minecraftArguments 兼容。

use std::collections::HashMap;

use crate::minecraft::rules::{rules_allow, Features};
use crate::minecraft::version::{Argument, VersionJson};
use crate::util::split_args;

pub type Vars = HashMap<&'static str, String>;

/// 替换 ${key}；未知变量保持原样
pub fn substitute(s: &str, vars: &Vars) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("${");
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn expand(args: &[Argument], features: &Features, vars: &Vars) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(substitute(s, vars)),
            Argument::Conditional { rules, value } => {
                if rules_allow(Some(rules), features) {
                    out.extend(value.values().iter().map(|v| substitute(v, vars)));
                }
            }
        }
    }
    out
}

/// 1.12.2 及更早版本（没有 arguments.jvm）时使用的默认 JVM 参数
pub fn legacy_jvm_args(vars: &Vars) -> Vec<String> {
    [
        "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump",
        "-Djava.library.path=${natives_directory}",
        "-Dminecraft.launcher.brand=${launcher_name}",
        "-Dminecraft.launcher.version=${launcher_version}",
        "-cp",
        "${classpath}",
    ]
    .iter()
    .map(|s| substitute(s, vars))
    .collect()
}

pub fn jvm_args(version: &VersionJson, features: &Features, vars: &Vars) -> Vec<String> {
    let from_json = version
        .arguments
        .as_ref()
        .map(|a| a.jvm.as_slice())
        .unwrap_or(&[]);
    if from_json.is_empty() {
        legacy_jvm_args(vars)
    } else {
        let mut args = expand(from_json, features, vars);
        // 部分第三方 JSON 的 jvm 参数里没有 classpath，补上
        if !args.iter().any(|a| a == "-cp" || a == "-classpath") {
            args.push("-cp".into());
            args.push(vars.get("classpath").cloned().unwrap_or_default());
        }
        if !args.iter().any(|a| a.starts_with("-Djava.library.path=")) {
            args.push(format!(
                "-Djava.library.path={}",
                vars.get("natives_directory").cloned().unwrap_or_default()
            ));
        }
        args
    }
}

/// 返回 (游戏参数, 是否为旧版参数格式)
pub fn game_args(version: &VersionJson, features: &Features, vars: &Vars) -> (Vec<String>, bool) {
    if let Some(a) = version.arguments.as_ref().filter(|a| !a.game.is_empty()) {
        return (expand(&a.game, features, vars), false);
    }
    let legacy = version.minecraft_arguments.clone().unwrap_or_default();
    (
        split_args(&legacy).iter().map(|s| substitute(s, vars)).collect(),
        true,
    )
}

/// 生成 Java @argfile 内容（Java 9+），每个参数一行并加引号
pub fn argfile_content(args: &[String]) -> String {
    args.iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_vars() {
        let mut v = Vars::new();
        v.insert("auth_player_name", "Steve".into());
        assert_eq!(substitute("--username=${auth_player_name}", &v), "--username=Steve");
        assert_eq!(substitute("${unknown}x", &v), "${unknown}x");
        assert_eq!(substitute("${broken", &v), "${broken");
    }
}
