//! 离线账户 UUID：与 Java `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)` 一致（MD5，版本 3）。

use md5::{Digest, Md5};

pub fn offline_uuid(name: &str) -> String {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest);
    // 版本号 3
    b[6] = (b[6] & 0x0f) | 0x30;
    // IETF 变体
    b[8] = (b[8] & 0x3f) | 0x80;
    hex::encode(b)
}

/// 32 位无连字符 UUID → 带连字符格式
pub fn dashed(uuid: &str) -> String {
    let u = uuid.replace('-', "");
    if u.len() != 32 {
        return uuid.to_string();
    }
    format!("{}-{}-{}-{}-{}", &u[0..8], &u[8..12], &u[12..16], &u[16..20], &u[20..32])
}

pub fn validate_offline_name(name: &str) -> anyhow::Result<()> {
    let n = name.trim();
    if n.is_empty() {
        anyhow::bail!("玩家名不能为空");
    }
    if n.chars().count() > 16 {
        anyhow::bail!("玩家名不能超过 16 个字符");
    }
    if n.chars().any(|c| c.is_whitespace()) {
        anyhow::bail!("玩家名不能包含空格");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_offline_uuid() {
        // Java: UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes(UTF_8))
        assert_eq!(dashed(&offline_uuid("Notch")), "b50ad385-829d-3141-a216-7e7d7539ba7f");
    }
}
