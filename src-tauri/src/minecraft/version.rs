//! 版本 JSON 的数据结构、inheritsFrom 合并与读取。

use std::collections::HashMap;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use super::rules::Rule;
use crate::paths::GameDirs;
use crate::util::read_json;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub jar: Option<String>,
    #[serde(rename = "type", default)]
    pub version_type: Option<String>,
    #[serde(default)]
    pub main_class: Option<String>,
    /// 1.12.2 及更早版本使用的整串游戏参数
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    /// 1.13+ 的结构化参数
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: Option<HashMap<String, DownloadInfo>>,
    #[serde(default)]
    pub java_version: Option<JavaVersionReq>,
    #[serde(default)]
    pub release_time: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: ArgValue,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    pub fn values(&self) -> Vec<String> {
        match self {
            ArgValue::One(s) => vec![s.clone()],
            ArgValue::Many(v) => v.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    /// Maven 仓库根地址（Forge / Fabric 风格的库会给出）
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<ExtractRule>,
    /// Fabric / Quilt 风格的库直接在顶层给出校验值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ExtractRule {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DownloadInfo {
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionReq {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub major_version: Option<u32>,
}

impl VersionJson {
    /// 返回 (运行时组件名, Java 主版本)。缺省为 jre-legacy / 8。
    pub fn java_requirement(&self) -> (Option<String>, u32) {
        match &self.java_version {
            Some(j) => (
                j.component.clone().filter(|c| !c.is_empty()),
                j.major_version.unwrap_or(8),
            ),
            None => (Some("jre-legacy".to_string()), 8),
        }
    }

    /// 客户端 jar 所属的版本 id（加载器实例的 jar 通常来自父版本）
    pub fn jar_id(&self) -> String {
        self.jar.clone().unwrap_or_else(|| self.id.clone())
    }
}

/// 子版本覆盖父版本：库和参数拼接（子在前），其余字段子版本优先。
pub fn merge(child: VersionJson, parent: VersionJson) -> VersionJson {
    let mut libraries = child.libraries;
    libraries.extend(parent.libraries);

    let arguments = match (child.arguments, parent.arguments) {
        (None, None) => None,
        (Some(c), None) => Some(c),
        (None, Some(p)) => Some(p),
        (Some(c), Some(p)) => {
            let mut game = p.game;
            game.extend(c.game);
            let mut jvm = p.jvm;
            jvm.extend(c.jvm);
            Some(Arguments { game, jvm })
        }
    };

    VersionJson {
        id: child.id,
        inherits_from: parent.inherits_from,
        jar: child.jar.or(parent.jar).or(Some(parent.id)),
        version_type: child.version_type.or(parent.version_type),
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        libraries,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        release_time: child.release_time.or(parent.release_time),
    }
}

/// 读取实例版本 JSON 并递归合并 inheritsFrom 链。
pub fn load_resolved(dirs: &GameDirs, name: &str) -> anyhow::Result<VersionJson> {
    let path = dirs.version_json(name);
    let mut current: VersionJson =
        read_json(&path).with_context(|| format!("实例 {name} 的版本文件缺失或损坏"))?;
    // 实例 JSON 的 id 可能与文件夹不一致（从其他启动器复制而来），以文件夹为准
    current.id = name.to_string();
    let mut depth = 0;
    while let Some(parent_id) = current.inherits_from.clone() {
        depth += 1;
        if depth > 8 {
            bail!("inheritsFrom 层级过深，可能存在循环引用");
        }
        let parent_path = dirs.version_json(&parent_id);
        let parent: VersionJson = read_json(&parent_path)
            .with_context(|| format!("找不到父版本 {parent_id}（{}）", parent_path.display()))?;
        current = merge(current, parent);
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modern_arguments() {
        let json = r#"{
            "id": "1.20.1",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "game": ["--username", "${auth_player_name}",
                    {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],
                     "value":["--width","${resolution_width}"]}],
                "jvm": [{"rules":[{"action":"allow","os":{"name":"windows"}}],
                     "value":"-XX:HeapDumpPath=x"}, "-cp", "${classpath}"]
            },
            "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17}
        }"#;
        let v: VersionJson = serde_json::from_str(json).unwrap();
        assert_eq!(v.arguments.as_ref().unwrap().game.len(), 3);
        assert_eq!(v.java_requirement(), (Some("java-runtime-gamma".into()), 17));
    }

    #[test]
    fn merge_inherits() {
        let parent = VersionJson { id: "1.20.1".into(), main_class: Some("A".into()), ..Default::default() };
        let child = VersionJson {
            id: "fabric".into(),
            inherits_from: Some("1.20.1".into()),
            main_class: Some("B".into()),
            ..Default::default()
        };
        let m = merge(child, parent);
        assert_eq!(m.main_class.as_deref(), Some("B"));
        assert_eq!(m.jar.as_deref(), Some("1.20.1"));
        assert!(m.inherits_from.is_none());
    }
}
