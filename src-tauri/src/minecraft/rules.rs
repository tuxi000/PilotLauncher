//! 版本 JSON 中 `rules` 的判断（仅考虑 Windows x64）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

/// 启动特性开关。未列出的特性一律视为 false（如 is_demo_user、has_quick_plays_support）。
#[derive(Clone, Debug, Default)]
pub struct Features {
    pub has_custom_resolution: bool,
}

impl Features {
    pub fn get(&self, key: &str) -> bool {
        match key {
            "has_custom_resolution" => self.has_custom_resolution,
            _ => false,
        }
    }
}

/// Windows 11 在 Java 中报告的 os.version 为 10.0
const OS_VERSION: &str = "10.0";

fn version_matches(pattern: &str, actual: &str) -> bool {
    // 官方 JSON 中只出现过 "^10\\." 这类前缀匹配，这里做简化处理
    let p = pattern.trim_start_matches('^').trim_end_matches('$').replace("\\.", ".");
    if p.chars().any(|c| "*+([|\\".contains(c)) {
        return true;
    }
    actual.starts_with(&p)
}

pub fn os_matches(os: &OsRule) -> bool {
    if let Some(name) = &os.name {
        if name != "windows" {
            return false;
        }
    }
    if let Some(arch) = &os.arch {
        // Mojang 用 "x86" 表示 32 位
        if !matches!(arch.as_str(), "x86_64" | "amd64" | "x64") {
            return false;
        }
    }
    if let Some(ver) = &os.version {
        if !version_matches(ver, OS_VERSION) {
            return false;
        }
    }
    true
}

/// 规则判断：没有规则时允许；有规则时默认禁止，最后一条匹配的规则决定结果。
pub fn rules_allow(rules: Option<&Vec<Rule>>, features: &Features) -> bool {
    let rules = match rules {
        Some(r) if !r.is_empty() => r,
        _ => return true,
    };
    let mut allow = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().map(os_matches).unwrap_or(true);
        let feat_ok = rule
            .features
            .as_ref()
            .map(|f| f.iter().all(|(k, v)| features.get(k) == *v))
            .unwrap_or(true);
        if os_ok && feat_ok {
            allow = rule.action == "allow";
        }
    }
    allow
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(action: &str, os: Option<&str>) -> Rule {
        Rule {
            action: action.into(),
            os: os.map(|n| OsRule { name: Some(n.into()), ..Default::default() }),
            features: None,
        }
    }

    #[test]
    fn classic_osx_disallow() {
        // 旧版 lwjgl：允许全部，但禁止 osx
        let rules = vec![rule("allow", None), rule("disallow", Some("osx"))];
        assert!(rules_allow(Some(&rules), &Features::default()));
        // 只允许 osx
        let rules = vec![rule("allow", Some("osx"))];
        assert!(!rules_allow(Some(&rules), &Features::default()));
    }

    #[test]
    fn version_pattern() {
        assert!(version_matches("^10\\.", "10.0"));
        assert!(!version_matches("^6\\.1", "10.0"));
    }
}
