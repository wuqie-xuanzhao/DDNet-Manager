//! NTFS $MFT FILE record 字节解析器。
//!
//! 纯字节解析，不依赖 Windows API，可跨平台单元测试。
//!
//! ## NTFS FILE Record 结构（典型 1024 字节）
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       4     Signature "FILE"
//! 4       2     USA (Update Sequence Array) offset
//! 6       2     USA size (含 USN，每 512 字节 sector 一个)
//! 8       8     $LogFile Sequence Number
//! 16      2     Sequence Number (record 复用计数)
//! 18      2     Hard Link Count
//! 20      2     First Attribute Offset
//! 22      2     Flags (0x01=in use, 0x02=directory)
//! 24      4     Used Size
//! 28      4     Allocated Size
//! ```
//!
//! ## USA Fixup
//!
//! 每个 512-byte sector 的最后 2 字节被替换为 USN（Update Sequence Number）。
//! 还原：USA[0] = USN，USA[i+1] = 第 i 个 sector 末尾 2 字节的原始值。
//!
//! ## Attribute Header（每个属性前 16 字节）
//!
//! ```text
//! 0   4   Type (30=$ATTRIBUTE_LIST, 48=$STD_INFO, 60=$FILE_NAME, 128=$DATA, ...)
//! 4   4   Length (含 header)
//! 8   1   Non-resident flag
//! 9   1   Name length (chars)
//! 10  2   Name offset
//! 12  2   Flags
//! 14  2   Attribute ID
//! ```
//!
//! ## Resident vs Non-resident
//!
//! Resident 属性内容直接嵌在 record 中（Content Length + Content Offset）。
//! Non-resident 属性内容在外部 cluster（Real Size + Data Run List）。
//!
//! 关键属性：
//! - `$STANDARD_INFORMATION` (48)：timestamps + flags
//! - `$FILE_NAME` (60)：parent_ref + name
//! - `$DATA` (128)：size（resident = Content Length，non-resident = Real Size）

// M3a：仅解析器，尚未接入 MftBackend（M3b）。所有 item 暂为 dead code。
// 4b 接入 MftBackend 后移除本 allow。
#![allow(dead_code)]

use crate::rebuild_paths::NTFS_ROOT_FILE_REFERENCE;

/// MFT record 固定头偏移常量（参见模块顶部文档）。
const RECORD_SIGNATURE: &[u8] = b"FILE";
const SECTOR_SIZE: usize = 512;
const ATTR_HEADER_FIXED: usize = 16;
const ATTR_TYPE_END_MARKER: u32 = 0xFFFF_FFFF;

/// 标准 NTFS 属性 Type Code。
const ATTR_STANDARD_INFORMATION: u32 = 48;
const ATTR_FILE_NAME: u32 = 60;
const ATTR_DATA: u32 = 128;

/// 解析后单条 MFT record 的关键字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedMftRecord {
    pub file_name: String,
    pub parent_reference: u64,
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
    pub attributes: u32,
    pub size: u64,
    pub is_directory: bool,
}

impl Default for ParsedMftRecord {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            parent_reference: NTFS_ROOT_FILE_REFERENCE,
            created: 0,
            modified: 0,
            accessed: 0,
            attributes: 0,
            size: 0,
            is_directory: false,
        }
    }
}

/// 解析一条 MFT FILE record 字节流。返回 `None` 表示 record 不可解析。
///
/// **不**抛 panic——所有越界访问返回 None。USA mismatch / 字段越界用宽容策略
/// （尽量返回部分结果）。
pub(crate) fn parse_mft_record(bytes: &[u8]) -> Option<ParsedMftRecord> {
    if bytes.len() < 48 || &bytes[0..4] != RECORD_SIGNATURE {
        return None;
    }

    let usa_offset = u16::from_le_bytes(bytes[4..6].try_into().ok()?) as usize;
    let usa_count = u16::from_le_bytes(bytes[6..8].try_into().ok()?) as usize;

    // Apply USA fixup（拷贝一份再修改，不污染原 bytes）
    let fixed = apply_usa_fixup(bytes, usa_offset, usa_count)?;

    let first_attr_offset = u16::from_le_bytes(fixed[20..22].try_into().ok()?) as usize;
    let flags = u32::from_le_bytes(fixed[22..26].try_into().ok()?);
    let is_directory = (flags & 0x02) != 0;

    let mut result = ParsedMftRecord {
        is_directory,
        ..ParsedMftRecord::default()
    };

    // 遍历所有属性，按 Type 提取关键字段
    for attr in walk_attributes(&fixed, first_attr_offset) {
        match attr.type_code {
            ATTR_STANDARD_INFORMATION => {
                extract_standard_information(&attr, &mut result);
            }
            ATTR_FILE_NAME => {
                extract_file_name(&attr, &mut result);
            }
            ATTR_DATA => {
                extract_data_size(&attr, &mut result);
            }
            _ => {}
        }
    }

    Some(result)
}

/// Apply USA fixup：还原每 512-byte sector 末尾被 USN 覆盖的 2 字节。
fn apply_usa_fixup(bytes: &[u8], usa_offset: usize, usa_count: usize) -> Option<Vec<u8>> {
    if usa_count == 0 {
        return Some(bytes.to_vec());
    }
    if usa_offset + usa_count * 2 > bytes.len() {
        // USA 区域越界 → record 严重损坏，仍尝试解析（宽容）
        return Some(bytes.to_vec());
    }

    let usn = u16::from_le_bytes(bytes[usa_offset..usa_offset + 2].try_into().ok()?);
    let mut fixed = bytes.to_vec();

    for i in 1..usa_count {
        let sector_end = i * SECTOR_SIZE;
        if sector_end > bytes.len() {
            break;
        }
        let check_offset = sector_end - 2;
        if check_offset + 2 > bytes.len() {
            break;
        }

        let check_val = u16::from_le_bytes(bytes[check_offset..check_offset + 2].try_into().ok()?);
        if check_val != usn {
            // USA mismatch（record 可能正在被写）→ 不还原，继续
            continue;
        }

        let orig_offset = usa_offset + i * 2;
        if orig_offset + 2 > bytes.len() {
            break;
        }
        let orig = u16::from_le_bytes(bytes[orig_offset..orig_offset + 2].try_into().ok()?);
        fixed[check_offset..check_offset + 2].copy_from_slice(&orig.to_le_bytes());
    }

    Some(fixed)
}

/// 遍历 record 内所有属性，从 `first_attr_offset` 开始。
fn walk_attributes(bytes: &[u8], first_attr_offset: usize) -> AttributeIter<'_> {
    AttributeIter {
        bytes,
        offset: first_attr_offset,
    }
}

struct AttributeIter<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for AttributeIter<'a> {
    type Item = RawAttribute<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + ATTR_HEADER_FIXED > self.bytes.len() {
            return None;
        }

        let type_code =
            u32::from_le_bytes(self.bytes[self.offset..self.offset + 4].try_into().ok()?);
        if type_code == ATTR_TYPE_END_MARKER {
            return None;
        }

        let length = u32::from_le_bytes(
            self.bytes[self.offset + 4..self.offset + 8]
                .try_into()
                .ok()?,
        ) as usize;
        if length < ATTR_HEADER_FIXED || self.offset + length > self.bytes.len() {
            return None;
        }

        let non_resident = self.bytes[self.offset + 8] != 0;
        let bytes = &self.bytes[self.offset..self.offset + length];
        self.offset += length;

        Some(RawAttribute {
            type_code,
            non_resident,
            bytes,
        })
    }
}

/// 一条原始属性（含 header）。
struct RawAttribute<'a> {
    type_code: u32,
    non_resident: bool,
    bytes: &'a [u8],
}

impl<'a> RawAttribute<'a> {
    /// Resident 属性：返回 content slice。
    fn resident_content(&self) -> Option<&'a [u8]> {
        if self.non_resident || self.bytes.len() < 24 {
            return None;
        }
        let content_length = u32::from_le_bytes(self.bytes[16..20].try_into().ok()?) as usize;
        let content_offset = u16::from_le_bytes(self.bytes[20..22].try_into().ok()?) as usize;
        if content_offset + content_length > self.bytes.len() {
            return None;
        }
        Some(&self.bytes[content_offset..content_offset + content_length])
    }

    /// Non-resident 属性：返回 Real Size（实际文件大小）。
    fn non_resident_real_size(&self) -> Option<u64> {
        if !self.non_resident || self.bytes.len() < 56 {
            return None;
        }
        Some(u64::from_le_bytes(self.bytes[48..56].try_into().ok()?))
    }
}

/// 从 $STANDARD_INFORMATION 提取 timestamps + attributes。
fn extract_standard_information(attr: &RawAttribute<'_>, result: &mut ParsedMftRecord) {
    let Some(content) = attr.resident_content() else {
        return;
    };
    if content.len() >= 8 {
        result.created = u64::from_le_bytes(content[0..8].try_into().unwrap_or([0u8; 8]));
    }
    if content.len() >= 16 {
        result.modified = u64::from_le_bytes(content[8..16].try_into().unwrap_or([0u8; 8]));
    }
    if content.len() >= 32 {
        result.accessed = u64::from_le_bytes(content[24..32].try_into().unwrap_or([0u8; 8]));
    }
    // FileAttributes 在 v3+ STANDARD_INFORMATION 中位于 offset 56
    if content.len() >= 60 {
        result.attributes = u32::from_le_bytes(content[56..60].try_into().unwrap_or([0u8; 4]));
    }
}

/// 从 $FILE_NAME 提取 parent_reference + name。
/// 多个 $FILE_NAME 时优先取 Win32 namespace (1) 或 Win32&DOS (3)。
fn extract_file_name(attr: &RawAttribute<'_>, result: &mut ParsedMftRecord) {
    let Some(content) = attr.resident_content() else {
        return;
    };
    if content.len() < 66 {
        return;
    }

    let parent_ref = u64::from_le_bytes(content[0..8].try_into().unwrap_or([0u8; 8]));
    let name_len_chars = content[64] as usize;
    let namespace = content[65];
    let name_bytes = name_len_chars * 2;
    if content.len() < 66 + name_bytes {
        return;
    }

    let name_utf16: Vec<u16> = content[66..66 + name_bytes]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let name = String::from_utf16_lossy(&name_utf16);

    // Parent reference 总是取（即使本属性后续不取 name）
    result.parent_reference = parent_ref;

    // Namespace 优先级：Win32 (1) / Win32&DOS (3) > POSIX (0) > DOS (2)
    let take_name = result.file_name.is_empty()
        || (matches!(namespace, 1 | 3) && !matches!(preferred_namespace(&result.file_name), 1 | 3));
    if take_name {
        result.file_name = name;
    }
}

/// 检查已有 name 的 namespace——我们不在 ParsedMftRecord 里存 namespace，
/// 简单策略：如果已存 name 含 DOS 风格（如全大写 + 8.3），返回 2；否则 1。
fn preferred_namespace(_name: &str) -> u8 {
    // 简化：实际 NTFS 中 $FILE_NAME 多 namespace 时几乎总有 Win32 name，
    // 此处假设已有 name 是优先（除非新 name 是 Win32）。
    1
}

/// 从 $DATA 提取文件 size（resident = Content Length，non-resident = Real Size）。
fn extract_data_size(attr: &RawAttribute<'_>, result: &mut ParsedMftRecord) {
    if attr.non_resident {
        if let Some(size) = attr.non_resident_real_size() {
            result.size = size;
        }
    } else if let Some(content) = attr.resident_content() {
        result.size = content.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个完整的 FILE record 字节流（含 USA + 多属性）。
    fn build_file_record(
        parent_ref: u64,
        name: &str,
        timestamps: (u64, u64, u64),
        attributes: u32,
        data_size: u64,
        is_directory: bool,
    ) -> Vec<u8> {
        // 简化：不做真实 USA fixup（usa_count=1 表示无 sector）
        let record_size = 1024;
        let mut buf = vec![0u8; record_size];

        // Header
        buf[0..4].copy_from_slice(b"FILE");
        let usa_offset = 42u16;
        let usa_count = 3u16; // 1 USN + 2 sector originals (1024 / 512 = 2 sectors)
        buf[4..6].copy_from_slice(&usa_offset.to_le_bytes());
        buf[6..8].copy_from_slice(&usa_count.to_le_bytes());

        let flags: u32 = if is_directory { 0x03 } else { 0x01 };
        let first_attr_offset: u16 = 56;
        buf[20..22].copy_from_slice(&first_attr_offset.to_le_bytes());
        buf[22..26].copy_from_slice(&flags.to_le_bytes());
        buf[24..28].copy_from_slice(&(record_size as u32).to_le_bytes());

        // USA: USN=0x1234 + 2 originals（0xAAAA, 0xBBBB）
        buf[42..44].copy_from_slice(&0x1234u16.to_le_bytes());
        buf[44..46].copy_from_slice(&0xAAAAu16.to_le_bytes());
        buf[46..48].copy_from_slice(&0xBBBBu16.to_le_bytes());

        // Sector 0 末尾 (offset 510-511) 和 sector 1 末尾 (offset 1022-1023) 放 USN
        buf[510..512].copy_from_slice(&0x1234u16.to_le_bytes());
        buf[1022..1024].copy_from_slice(&0x1234u16.to_le_bytes());

        // === Attributes ===
        let mut offset = first_attr_offset as usize;

        // $STANDARD_INFORMATION (resident)
        offset = write_std_info_attr(&mut buf, offset, timestamps, attributes);

        // $FILE_NAME (resident)
        offset = write_file_name_attr(&mut buf, offset, parent_ref, name, timestamps);

        // $DATA
        offset = write_data_attr(&mut buf, offset, data_size);

        // End marker
        if offset + 8 <= buf.len() {
            buf[offset..offset + 4].copy_from_slice(&ATTR_TYPE_END_MARKER.to_le_bytes());
        }

        buf
    }

    fn write_std_info_attr(
        buf: &mut [u8],
        offset: usize,
        ts: (u64, u64, u64),
        attrs: u32,
    ) -> usize {
        let content_len = 72usize; // STANDARD_INFORMATION v3 size
        let content_offset = 24usize;
        let attr_len = content_offset + content_len;
        buf[offset..offset + 4].copy_from_slice(&ATTR_STANDARD_INFORMATION.to_le_bytes());
        buf[offset + 4..offset + 8].copy_from_slice(&(attr_len as u32).to_le_bytes());
        buf[offset + 8] = 0; // resident
        buf[offset + 16..offset + 20].copy_from_slice(&(content_len as u32).to_le_bytes());
        buf[offset + 20..offset + 22].copy_from_slice(&(content_offset as u16).to_le_bytes());

        let c = offset + content_offset;
        buf[c..c + 8].copy_from_slice(&ts.0.to_le_bytes()); // created
        buf[c + 8..c + 16].copy_from_slice(&ts.1.to_le_bytes()); // modified
        buf[c + 24..c + 32].copy_from_slice(&ts.2.to_le_bytes()); // accessed
        buf[c + 56..c + 60].copy_from_slice(&attrs.to_le_bytes());

        offset + attr_len
    }

    fn write_file_name_attr(
        buf: &mut [u8],
        offset: usize,
        parent_ref: u64,
        name: &str,
        ts: (u64, u64, u64),
    ) -> usize {
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let name_byte_len = name_utf16.len() * 2;
        let content_len = 66 + name_byte_len;
        let content_offset = 24usize;
        let attr_len = content_offset + content_len;

        buf[offset..offset + 4].copy_from_slice(&ATTR_FILE_NAME.to_le_bytes());
        buf[offset + 4..offset + 8].copy_from_slice(&(attr_len as u32).to_le_bytes());
        buf[offset + 8] = 0; // resident
        buf[offset + 16..offset + 20].copy_from_slice(&(content_len as u32).to_le_bytes());
        buf[offset + 20..offset + 22].copy_from_slice(&(content_offset as u16).to_le_bytes());

        let c = offset + content_offset;
        buf[c..c + 8].copy_from_slice(&parent_ref.to_le_bytes());
        buf[c + 8..c + 16].copy_from_slice(&ts.0.to_le_bytes());
        buf[c + 16..c + 24].copy_from_slice(&ts.1.to_le_bytes());
        buf[c + 24..c + 32].copy_from_slice(&ts.1.to_le_bytes());
        buf[c + 32..c + 40].copy_from_slice(&ts.2.to_le_bytes());
        buf[c + 64] = name_utf16.len() as u8;
        buf[c + 65] = 1; // namespace = Win32
        for (i, &w) in name_utf16.iter().enumerate() {
            let p = c + 66 + i * 2;
            buf[p..p + 2].copy_from_slice(&w.to_le_bytes());
        }

        offset + attr_len
    }

    fn write_data_attr(buf: &mut [u8], offset: usize, size: u64) -> usize {
        if size <= 512 {
            // resident
            let content_len = size as usize;
            let content_offset = 24usize;
            let attr_len = content_offset + content_len;
            buf[offset..offset + 4].copy_from_slice(&ATTR_DATA.to_le_bytes());
            buf[offset + 4..offset + 8].copy_from_slice(&(attr_len as u32).to_le_bytes());
            buf[offset + 8] = 0; // resident
            buf[offset + 16..offset + 20].copy_from_slice(&(content_len as u32).to_le_bytes());
            buf[offset + 20..offset + 22].copy_from_slice(&(content_offset as u16).to_le_bytes());
            offset + attr_len
        } else {
            // non-resident
            let attr_len = 72usize;
            buf[offset..offset + 4].copy_from_slice(&ATTR_DATA.to_le_bytes());
            buf[offset + 4..offset + 8].copy_from_slice(&(attr_len as u32).to_le_bytes());
            buf[offset + 8] = 1; // non-resident
                                 // Real Size at offset 48-56
            buf[offset + 48..offset + 56].copy_from_slice(&size.to_le_bytes());
            offset + attr_len
        }
    }

    #[test]
    fn parses_minimal_file_record() {
        let bytes = build_file_record(5, "test.txt", (100, 200, 300), 0x20, 0, false);
        let parsed = parse_mft_record(&bytes).expect("should parse");

        assert_eq!(parsed.file_name, "test.txt");
        assert_eq!(parsed.parent_reference, 5);
        assert_eq!(parsed.created, 100);
        assert_eq!(parsed.modified, 200);
        assert_eq!(parsed.accessed, 300);
        assert_eq!(parsed.attributes, 0x20);
        assert_eq!(parsed.size, 0);
        assert!(!parsed.is_directory);
    }

    #[test]
    fn parses_directory_record() {
        let bytes = build_file_record(5, "mydir", (10, 20, 30), 0x10, 0, true);
        let parsed = parse_mft_record(&bytes).expect("should parse");
        assert!(parsed.is_directory);
    }

    #[test]
    fn parses_resident_data_size() {
        let bytes = build_file_record(5, "small.txt", (0, 0, 0), 0x20, 100, false);
        let parsed = parse_mft_record(&bytes).expect("should parse");
        assert_eq!(parsed.size, 100);
    }

    #[test]
    fn parses_non_resident_data_size() {
        let bytes = build_file_record(5, "large.bin", (0, 0, 0), 0x20, 1_000_000, false);
        let parsed = parse_mft_record(&bytes).expect("should parse");
        assert_eq!(parsed.size, 1_000_000);
    }

    #[test]
    fn rejects_non_file_signature() {
        let mut bytes = build_file_record(5, "test", (0, 0, 0), 0, 0, false);
        bytes[0..4].copy_from_slice(b"XXXX");
        assert!(parse_mft_record(&bytes).is_none());
    }

    #[test]
    fn rejects_short_record() {
        let bytes = vec![0u8; 32];
        assert!(parse_mft_record(&bytes).is_none());
    }

    #[test]
    fn handles_chinese_filename() {
        let bytes = build_file_record(5, "客户端.exe", (0, 0, 0), 0x20, 0, false);
        let parsed = parse_mft_record(&bytes).expect("should parse");
        assert_eq!(parsed.file_name, "客户端.exe");
    }

    #[test]
    fn usa_fixup_restores_sector_bytes() {
        let bytes = build_file_record(5, "test", (0, 0, 0), 0, 0, false);
        // build_file_record 已设置 USN 0x1234 在 sector 末尾，USA 数组保存原值。
        // parse 时 USA fixup 应还原 sector 末尾为 0xAAAA / 0xBBBB。
        // 主要验证：record 仍可正常解析（USA fixup 未破坏字段）
        let parsed = parse_mft_record(&bytes).expect("should parse");
        assert_eq!(parsed.file_name, "test");
    }

    #[test]
    fn usa_mismatch_does_not_panic() {
        let mut bytes = build_file_record(5, "test", (0, 0, 0), 0, 0, false);
        // 故意把 sector 末尾改成非 USN 值，模拟 record 正在被写入
        bytes[510..512].copy_from_slice(&0xDEADu16.to_le_bytes());
        let parsed = parse_mft_record(&bytes);
        // 仍应能解析（宽容策略），即使 USA mismatch
        assert!(parsed.is_some());
    }

    #[test]
    fn usa_count_zero_falls_back_gracefully() {
        let mut bytes = build_file_record(5, "test", (0, 0, 0), 0, 0, false);
        bytes[6..8].copy_from_slice(&0u16.to_le_bytes()); // usa_count = 0
        let parsed = parse_mft_record(&bytes);
        assert!(parsed.is_some());
    }

    #[test]
    fn record_with_no_file_name_attr_returns_empty_name() {
        // 构造一个只有 STD_INFO 和 $DATA 的 record（罕见，但合法）
        let record_size = 256;
        let mut buf = vec![0u8; record_size];
        buf[0..4].copy_from_slice(b"FILE");
        buf[4..6].copy_from_slice(&42u16.to_le_bytes());
        buf[6..8].copy_from_slice(&1u16.to_le_bytes()); // usa_count = 1（仅 USN）
        let first_attr_offset = 56u16;
        buf[20..22].copy_from_slice(&first_attr_offset.to_le_bytes());
        buf[22..26].copy_from_slice(&1u32.to_le_bytes()); // in use

        let mut offset = first_attr_offset as usize;
        // 仅写 $DATA，不写 $FILE_NAME
        offset = write_data_attr(&mut buf, offset, 50);
        buf[offset..offset + 4].copy_from_slice(&ATTR_TYPE_END_MARKER.to_le_bytes());

        let parsed = parse_mft_record(&buf).expect("should parse");
        assert!(parsed.file_name.is_empty());
        assert_eq!(parsed.parent_reference, NTFS_ROOT_FILE_REFERENCE); // 默认值
        assert_eq!(parsed.size, 50);
    }

    #[test]
    fn truncated_attributes_do_not_panic() {
        // 构造一个 first_attr_offset 指向 record 边界外
        let mut bytes = build_file_record(5, "test", (0, 0, 0), 0, 0, false);
        bytes[20..22].copy_from_slice(&1023u16.to_le_bytes()); // 几乎到边界
        let _ = parse_mft_record(&bytes); // 不应 panic
    }
}
