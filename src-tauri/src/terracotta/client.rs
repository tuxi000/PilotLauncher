//! Terracotta 本机 HTTP 接口（只监听 127.0.0.1，端口随机）的封装。
//!
//! 端点（以 v0.4.2 源码与官方文档 sites/docs/http-api.md 为准，全部为 GET）：
//! - `/meta`：版本信息
//! - `/state`：当前状态（waiting / host-scanning / host-starting / host-ok /
//!   guest-connecting / guest-starting / guest-ok / exception），`index` 单调递增
//! - `/state/ide`：回到待机（退出房间 / 关闭房间 / 清除异常）
//! - `/state/scanning?player=..`：以房主身份开始扫描本机局域网世界（仅 waiting 时生效）
//! - `/state/guesting?room=..&player=..`：以房客身份加入（邀请码无效或不在 waiting 时返回 400）
//! - `/log?fetch=true`：日志文件内容
//! - `/panic?peaceful=true`：正常退出进程

use std::time::Duration;

use anyhow::{anyhow, bail};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub machine_id: String,
    pub name: String,
    pub vendor: String,
    /// HOST（房主）/ LOCAL（本机）/ GUEST（其他房客）
    pub kind: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoomState {
    pub state: String,
    pub index: u64,
    /// 邀请码（host-starting / host-ok / guest-connecting / guest-starting）
    pub room: Option<String>,
    /// 房客直连地址（guest-ok），如 127.0.0.1 或 127.0.0.1:端口
    pub url: Option<String>,
    /// NAT 连接难度（guest-starting）：UNKNOWN / EASIEST / SIMPLE / MEDIUM / TOUGH
    pub difficulty: Option<String>,
    /// 异常类型（exception）：0~5
    pub exception_type: Option<i64>,
    pub profiles: Vec<Profile>,
}

#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub version: String,
    pub easytier: String,
}

fn base(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

pub fn parse_state(v: &Value) -> anyhow::Result<RoomState> {
    let state = str_field(v, "state").ok_or_else(|| anyhow!("联机组件返回的状态缺少 state 字段"))?;
    let profiles: Vec<Profile> = v
        .get("profiles")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some(Profile {
                        machine_id: p.get("machine_id")?.as_str()?.to_string(),
                        name: p.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        vendor: p.get("vendor").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        kind: p.get("kind").and_then(|x| x.as_str()).unwrap_or("GUEST").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(RoomState {
        state,
        index: v.get("index").and_then(|x| x.as_u64()).unwrap_or(0),
        room: str_field(v, "room"),
        url: str_field(v, "url"),
        difficulty: str_field(v, "difficulty"),
        exception_type: v.get("type").and_then(|x| x.as_i64()),
        profiles,
    })
}

pub async fn meta(http: &reqwest::Client, port: u16) -> anyhow::Result<Meta> {
    let v: Value = http
        .get(format!("{}/meta", base(port)))
        .timeout(Duration::from_millis(1500))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(Meta {
        version: str_field(&v, "version").unwrap_or_default(),
        easytier: str_field(&v, "easytier_version").unwrap_or_default(),
    })
}

pub async fn state(http: &reqwest::Client, port: u16) -> anyhow::Result<RoomState> {
    let v: Value = http
        .get(format!("{}/state", base(port)))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_state(&v)
}

pub async fn ide(http: &reqwest::Client, port: u16) -> anyhow::Result<()> {
    http.get(format!("{}/state/ide", base(port)))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn scanning(http: &reqwest::Client, port: u16, player: Option<&str>) -> anyhow::Result<()> {
    let mut query: Vec<(&str, String)> = Vec::new();
    if let Some(p) = player {
        query.push(("player", p.to_string()));
    }
    http.get(format!("{}/state/scanning", base(port)))
        .query(&query)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn guesting(http: &reqwest::Client, port: u16, room: &str, player: Option<&str>) -> anyhow::Result<()> {
    let mut query: Vec<(&str, String)> = vec![("room", room.to_string())];
    if let Some(p) = player {
        query.push(("player", p.to_string()));
    }
    let resp = http
        .get(format!("{}/state/guesting", base(port)))
        .query(&query)
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::BAD_REQUEST {
        bail!("联机组件拒绝了这个邀请码（格式无效，或当前不在空闲状态）");
    }
    resp.error_for_status()?;
    Ok(())
}

pub async fn log(http: &reqwest::Client, port: u16) -> anyhow::Result<String> {
    let text = http
        .get(format!("{}/log?fetch=true", base(port)))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(text)
}

/// 让进程正常退出（进程退出时连接会被直接断开，忽略错误）
pub async fn panic_peaceful(http: &reqwest::Client, port: u16) {
    let _ = http
        .get(format!("{}/panic?peaceful=true", base(port)))
        .timeout(Duration::from_millis(1500))
        .send()
        .await;
}

/// 同步版本的 GET（启动器退出时在事件循环线程里调用，不依赖异步运行时）
pub fn raw_get(port: u16, path: &str) {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(800)) {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(1200)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(800)));
        let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        if stream.write_all(req.as_bytes()).is_ok() {
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf);
        }
    }
}

/// 邀请码规范化与校验，规则与 Terracotta 的 `controller/rooms/scaffolding/room.rs` 一致：
/// `U/XXXX-XXXX-XXXX-XXXX`，字符集 0-9 A-Z 去掉 I、O（输入的 I、O 按 1、0 处理），不区分大小写，
/// 16 位 34 进制数（首字符为最低位）必须是 7 的倍数；允许夹在其他文字中间（例如整段聊天消息）。
pub fn normalize_room_code(input: &str) -> Option<String> {
    const CHARS: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    const LEN: usize = 21; // "U/XXXX-XXXX-XXXX-XXXX".len()
    fn lookup(c: u8) -> Option<u128> {
        let c = match c {
            b'I' => b'1',
            b'O' => b'0',
            other => other,
        };
        CHARS.iter().position(|&x| x == c).map(|i| i as u128)
    }

    let upper = input.trim().to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() < LEN {
        return None;
    }
    'outer: for w in bytes.windows(LEN) {
        if w[0] != b'U' || w[1] != b'/' {
            continue;
        }
        let body = &w[2..];
        let mut value: u128 = 0;
        for i in (0..19).rev() {
            if i == 4 || i == 9 || i == 14 {
                if body[i] != b'-' {
                    continue 'outer;
                }
            } else {
                match lookup(body[i]) {
                    Some(v) => value = value * 34 + v,
                    None => continue 'outer,
                }
            }
        }
        if value % 7 == 0 {
            let mut out = String::with_capacity(LEN);
            out.push_str("U/");
            for &b in body {
                out.push(match b {
                    b'I' => '1',
                    b'O' => '0',
                    other => other as char,
                });
            }
            return Some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_code() {
        // 由 Terracotta 的生成算法得到的合法邀请码
        let code = "U/BM9R-ENW7-ETMV-E74E";
        assert_eq!(normalize_room_code(code).as_deref(), Some(code));
        assert_eq!(normalize_room_code("房间：u/bm9r-enw7-etmv-e74e 快来").as_deref(), Some(code));
        assert!(normalize_room_code("U/BM9R-ENW7-ETMV-E741").is_none());
        assert!(normalize_room_code("U/1234").is_none());
    }

    #[test]
    fn parse_host_ok() {
        let v: Value = serde_json::from_str(
            r#"{"state":"host-ok","index":4,"room":"U/BM9R-ENW7-ETMV-E74E","profile_index":0,
               "profiles":[{"machine_id":"a","name":"Steve","vendor":"Terracotta","kind":"HOST"}]}"#,
        )
        .unwrap();
        let s = parse_state(&v).unwrap();
        assert_eq!(s.state, "host-ok");
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].kind, "HOST");
    }
}
