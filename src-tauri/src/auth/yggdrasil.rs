//! Yggdrasil 外置登录（authlib-injector 规范）。
//!
//! 规范参考：https://github.com/yushijinhun/authlib-injector/wiki
//! - API 地址指示（ALI）：请求用户填写的地址，若响应头含 `X-Authlib-Injector-API-Location`，
//!   则以其（相对于当前地址解析后）作为 API 根地址。
//! - 元数据：GET API 根地址，返回 JSON（meta / skinDomains / signaturePublickey）。

use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const ALI_HEADER: &str = "x-authlib-injector-api-location";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub api_root: String,
    pub server_name: Option<String>,
    #[serde(skip)]
    pub metadata_b64: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GameProfile {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub access_token: String,
    #[serde(default)]
    pub client_token: Option<String>,
    #[serde(default)]
    pub available_profiles: Vec<GameProfile>,
    #[serde(default)]
    pub selected_profile: Option<GameProfile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_message: Option<String>,
}

fn normalize_input(input: &str) -> String {
    let t = input.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        t.to_string()
    } else {
        format!("https://{t}")
    }
}

fn trim_root(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

/// ALI 检测 + 获取服务器元数据
pub async fn resolve_server(client: &reqwest::Client, input: &str) -> anyhow::Result<ServerInfo> {
    let url = normalize_input(input);
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .with_context(|| format!("无法连接认证服务器 {url}"))?;
    let final_url = resp.url().clone();
    let ali = resp
        .headers()
        .get(ALI_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let (root, body) = match ali {
        Some(loc) => {
            let indicated = final_url
                .join(&loc)
                .with_context(|| format!("API 地址指示无效：{loc}"))?
                .to_string();
            if trim_root(&indicated) == trim_root(&url) {
                (url.clone(), resp.error_for_status()?.bytes().await?.to_vec())
            } else {
                drop(resp);
                let body = client
                    .get(&indicated)
                    .timeout(Duration::from_secs(15))
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?
                    .to_vec();
                (indicated, body)
            }
        }
        None => (url.clone(), resp.error_for_status()?.bytes().await?.to_vec()),
    };

    let meta: Value = serde_json::from_slice(&body).map_err(|_| anyhow!("该地址不是有效的 Yggdrasil 认证服务器"))?;
    if meta.get("signaturePublickey").is_none() && meta.get("meta").is_none() {
        bail!("该地址不是有效的 Yggdrasil 认证服务器（缺少元数据）");
    }
    let server_name = meta
        .get("meta")
        .and_then(|m| m.get("serverName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(ServerInfo {
        api_root: trim_root(&root),
        server_name,
        metadata_b64: STANDARD.encode(&body),
    })
}

async fn post_json<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    url: &str,
    body: Value,
) -> anyhow::Result<T> {
    let resp = client
        .post(url)
        .json(&body)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .with_context(|| format!("请求 {url} 失败"))?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        let msg = serde_json::from_str::<ErrorResponse>(&text)
            .ok()
            .and_then(|e| e.error_message.or(e.error))
            .unwrap_or_else(|| text.chars().take(200).collect());
        bail!("认证服务器返回错误（{}）：{}", status.as_u16(), msg);
    }
    serde_json::from_str(&text).context("认证服务器响应格式错误")
}

pub async fn authenticate(
    client: &reqwest::Client,
    root: &str,
    username: &str,
    password: &str,
    client_token: &str,
) -> anyhow::Result<AuthResponse> {
    post_json(
        client,
        &format!("{root}/authserver/authenticate"),
        json!({
            "agent": { "name": "Minecraft", "version": 1 },
            "username": username,
            "password": password,
            "clientToken": client_token,
            "requestUser": true
        }),
    )
    .await
}

pub async fn refresh(
    client: &reqwest::Client,
    root: &str,
    access_token: &str,
    client_token: &str,
    select: Option<&GameProfile>,
) -> anyhow::Result<AuthResponse> {
    let mut body = json!({
        "accessToken": access_token,
        "clientToken": client_token,
        "requestUser": true
    });
    if let Some(p) = select {
        body["selectedProfile"] = json!({ "id": p.id, "name": p.name });
    }
    post_json(client, &format!("{root}/authserver/refresh"), body).await
}

/// 令牌有效返回 true（服务器返回 204）
pub async fn validate(
    client: &reqwest::Client,
    root: &str,
    access_token: &str,
    client_token: &str,
) -> anyhow::Result<bool> {
    let resp = client
        .post(format!("{root}/authserver/validate"))
        .json(&json!({ "accessToken": access_token, "clientToken": client_token }))
        .timeout(Duration::from_secs(15))
        .send()
        .await?;
    Ok(resp.status().is_success())
}
