//! PE VS_VERSION_INFO 资源解析。基于 `pelite` crate。
//!
//! VS_VERSION_INFO 是 PE 文件资源段 RT_VERSION (type 16) 的标准结构，含：
//! - VS_FIXEDFILEINFO：二进制版本号（file_version / product_version / 等）
//! - StringFileInfo：UTF-16 字符串对（CompanyName / ProductName / 等）
//!
//! 典型用法：从 DDNet.exe 拿到 CompanyName，判定是官方 DDNet Team 还是第三方。
//!
//! 兼容性：跨平台（pelite 是纯 Rust PE 解析库）。

use crate::options::VersionInfo;
use std::path::Path;

/// 从 PE 文件读取 VS_VERSION_INFO。
///
/// 失败返回 Err（非 PE / 资源缺失 / pelite 解析失败）。
/// 调用方按需 fallback（如 FileEntry.size 取文件大小，version_info 设 None）。
pub(crate) fn read_version_info(path: &Path) -> Result<VersionInfo, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse_version_info_from_bytes(&bytes)
}

/// 从内存字节流解析 PE VS_VERSION_INFO。抽出便于单测（用 fixture bytes）。
pub(crate) fn parse_version_info_from_bytes(bytes: &[u8]) -> Result<VersionInfo, String> {
    let file = pelite::PeFile::from_bytes(bytes).map_err(|e| format!("pelite parse: {e}"))?;

    let resources = match file.resources() {
        Ok(r) => r,
        Err(e) => return Err(format!("resources directory: {e}")),
    };

    let version = match resources.version_info() {
        Ok(v) => v,
        Err(e) => return Err(format!("RT_VERSION resource: {e}")),
    };

    let mut vi = VersionInfo::default();

    // 用第一个 translation 的 Language 拿字符串对。
    // 多 translation PE（中英双版）目前取第一个，调用方按业务需求自行 fallback。
    // v0.2 可加优先级匹配（en-US > zh-CN > first）。
    if let Some(&lang) = version.translation().first() {
        version.strings(lang, |key, value| match key {
            "CompanyName" => vi.company_name = Some(value.to_string()),
            "ProductName" => vi.product_name = Some(value.to_string()),
            "FileDescription" => vi.file_description = Some(value.to_string()),
            "FileVersion" => vi.file_version = Some(value.to_string()),
            "ProductVersion" => vi.product_version = Some(value.to_string()),
            "OriginalFilename" => vi.original_filename = Some(value.to_string()),
            _ => {}
        });
    }

    // VS_FIXEDFILEINFO 二进制版本（StringFileInfo 没有时 fallback）
    if let Some(fixed) = version.fixed() {
        let fv = &fixed.dwFileVersion;
        let pv = &fixed.dwProductVersion;
        if vi.file_version.is_none() {
            vi.file_version = Some(format!(
                "{}.{}.{}.{}",
                fv.Major, fv.Minor, fv.Build, fv.Patch
            ));
        }
        if vi.product_version.is_none() {
            vi.product_version = Some(format!(
                "{}.{}.{}.{}",
                pv.Major, pv.Minor, pv.Build, pv.Patch
            ));
        }
    }

    Ok(vi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pe_bytes() {
        let bytes = b"not a PE file";
        assert!(parse_version_info_from_bytes(bytes).is_err());
    }

    #[test]
    fn rejects_empty_bytes() {
        let bytes = &[];
        assert!(parse_version_info_from_bytes(bytes).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn real_notepad_exe_has_company_name() {
        let path = Path::new("C:\\Windows\\System32\\notepad.exe");
        if !path.exists() {
            return;
        }
        let vi = read_version_info(path).expect("should parse notepad.exe version info");
        assert!(
            vi.company_name
                .as_deref()
                .map(|c| c.contains("Microsoft"))
                .unwrap_or(false),
            "CompanyName should contain 'Microsoft', got: {:?}",
            vi.company_name
        );
    }

    proptest::proptest! {
        /// Fuzz：任意字节流不应让 parse_version_info_from_bytes panic。
        #[test]
        fn parse_version_info_never_panics(
            bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..4096),
        ) {
            let _ = parse_version_info_from_bytes(&bytes);
        }

        /// Fuzz：以 PE 签名 MZ 起头也不应 panic（pelite 会尝试完整解析）。
        #[test]
        fn parse_version_info_with_mz_signature_never_panics(
            mut bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 64..4096),
        ) {
            bytes[0..2].copy_from_slice(b"MZ");
            let _ = parse_version_info_from_bytes(&bytes);
        }
    }
}
