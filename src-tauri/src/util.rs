//! 通用工具函数：错误转换、时间戳、哈希、JSON 读写、名称校验。

use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha1::{Digest, Sha1};

/// 所有 Tauri 命令统一返回的结果类型（错误以中文字符串返回给前端）。
pub type CmdResult<T> = Result<T, String>;

pub trait IntoCmd<T> {
    fn cmd(self) -> CmdResult<T>;
}

impl<T> IntoCmd<T> for anyhow::Result<T> {
    fn cmd(self) -> CmdResult<T> {
        // {:#} 会把 context 链一并输出，方便定位问题
        self.map_err(|e| format!("{:#}", e))
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Sha256;
    hex::encode(Sha256::digest(bytes))
}

/// 在阻塞线程池里计算文件 SHA-1，避免卡住异步运行时。
pub async fn file_sha1(path: &Path) -> anyhow::Result<String> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let mut f = std::fs::File::open(&p)?;
        let mut h = Sha1::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Ok(hex::encode(h.finalize()))
    })
    .await?
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let bytes = std::fs::read(path).with_context(|| format!("读取文件失败：{}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .with_context(|| format!("解析 JSON 失败：{}", path.display()))?;
    Ok(value)
}

/// 先写临时文件再改名，避免写到一半崩溃导致配置损坏。
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("写入文件失败：{}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("替换文件失败：{}", path.display()))?;
    Ok(())
}

/// 校验实例名称：必须是合法的 Windows 文件夹名，且不能破坏 classpath。
pub fn validate_instance_name(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        bail!("实例名称不能为空");
    }
    if name.trim() != name {
        bail!("实例名称首尾不能包含空格");
    }
    if name.chars().count() > 64 {
        bail!("实例名称不能超过 64 个字符");
    }
    // ';' 是 Windows classpath 分隔符，'!' 会破坏 jar URL，一并禁止
    const ILLEGAL: &str = "\\/:*?\"<>|;!";
    if name.chars().any(|c| ILLEGAL.contains(c) || c.is_control()) {
        bail!("实例名称不能包含以下字符：\\ / : * ? \" < > | ; !");
    }
    if name.ends_with('.') {
        bail!("实例名称不能以 . 结尾");
    }
    let upper = name.to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&upper.as_str()) {
        bail!("“{}” 是 Windows 保留名称，请换一个", name);
    }
    Ok(())
}

/// 按空白切分参数，支持用双引号包住含空格的参数。
pub fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut has_token = false;
    for c in input.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quote => {
                if has_token {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_works() {
        assert_eq!(
            split_args(r#"-Xss1M  "-Dfoo=a b" -Dbar"#),
            vec!["-Xss1M", "-Dfoo=a b", "-Dbar"]
        );
    }

    #[test]
    fn instance_name_rules() {
        assert!(validate_instance_name("1.20.1 生存").is_ok());
        assert!(validate_instance_name("a;b").is_err());
        assert!(validate_instance_name("CON").is_err());
        assert!(validate_instance_name(" x").is_err());
    }
}
