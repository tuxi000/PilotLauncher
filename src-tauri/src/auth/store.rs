//! 账户存储（accounts.json）。
//!
//! 注意：令牌以明文保存在本机用户目录中，不保存密码。

use std::path::PathBuf;

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::util::{read_json, write_json_atomic};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Offline,
    Yggdrasil,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// 本地唯一 id
    pub id: String,
    pub kind: AccountKind,
    /// 角色名
    pub name: String,
    /// 角色 UUID（无连字符）
    pub uuid: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub client_token: String,
    /// Yggdrasil API 根地址
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default)]
    pub server_name: Option<String>,
    /// 登录用的邮箱 / 用户名
    #[serde(default)]
    pub username: Option<String>,
    /// 服务器元数据（base64），用于 -Dauthlibinjector.yggdrasil.prebuilt
    #[serde(default)]
    pub metadata_b64: Option<String>,
    #[serde(default)]
    pub created_at: u64,
}

/// 发给前端的账户信息（不含令牌）
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub kind: AccountKind,
    pub name: String,
    pub uuid: String,
    pub server_url: Option<String>,
    pub server_name: Option<String>,
    pub username: Option<String>,
}

impl From<&Account> for AccountView {
    fn from(a: &Account) -> Self {
        Self {
            id: a.id.clone(),
            kind: a.kind,
            name: a.name.clone(),
            uuid: a.uuid.clone(),
            server_url: a.server_url.clone(),
            server_name: a.server_name.clone(),
            username: a.username.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct StoreFile {
    #[serde(default)]
    accounts: Vec<Account>,
}

pub struct AccountStore {
    path: PathBuf,
    pub accounts: Vec<Account>,
}

impl AccountStore {
    pub fn load(path: PathBuf) -> Self {
        let file: StoreFile = read_json(&path).unwrap_or_default();
        Self { path, accounts: file.accounts }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        write_json_atomic(
            &self.path,
            &StoreFile { accounts: self.accounts.clone() },
        )
    }

    pub fn views(&self) -> Vec<AccountView> {
        self.accounts.iter().map(AccountView::from).collect()
    }

    pub fn get(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// 新增或替换（同类型、同服务器、同 UUID 视为同一账户）
    pub fn upsert(&mut self, mut account: Account) -> anyhow::Result<Account> {
        if let Some(existing) = self.accounts.iter_mut().find(|a| {
            a.kind == account.kind && a.uuid == account.uuid && a.server_url == account.server_url
        }) {
            account.id = existing.id.clone();
            account.created_at = existing.created_at;
            *existing = account.clone();
        } else {
            self.accounts.push(account.clone());
        }
        self.save()?;
        Ok(account)
    }

    pub fn update(&mut self, account: Account) -> anyhow::Result<()> {
        match self.accounts.iter_mut().find(|a| a.id == account.id) {
            Some(a) => *a = account,
            None => bail!("账户不存在"),
        }
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> anyhow::Result<()> {
        self.accounts.retain(|a| a.id != id);
        self.save()
    }
}
