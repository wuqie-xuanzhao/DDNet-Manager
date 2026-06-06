use sha2::{Digest, Sha256};

/// 计算输入字节的 SHA-256 小写十六进制摘要。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_hex_matches_known_value() {
        assert_eq!(
            sha256_hex(b"ddnet-manager"),
            "739340afd53a209817636fca6d95d15abba5e236a11e49ff33e810111f00a55e"
        );
    }
}
