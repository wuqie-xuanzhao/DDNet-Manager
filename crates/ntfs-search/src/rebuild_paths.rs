//! NTFS 路径重建算法（MFT / USN backend 共用）。
//!
//! 输入：扫盘阶段收集到的 record map（FileRef → 文件名 + 父 FileRef）。
//! 输出：每个 record 的完整路径。
//!
//! 算法：对每个 FileRef，从自身向上递归到 NTFS 根（FileRef = 5），沿途拼接 path。
//! 防御点：
//! - 循环依赖（parent_ref 链中遇到自己）→ `RebuildError::CycleDetected`
//! - 悬空 parent_ref（不在 map 中，通常由杀软实时改 MFT 导致快照不一致）
//!   → `RebuildError::StaleParent`，调用方应 emit `EntryError` 跳过
//! - 限深 256 层（防恶意构造数据导致栈溢出）

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// NTFS 根目录的 FileReferenceNumber 固定为 5。
pub const NTFS_ROOT_FILE_REFERENCE: u64 = 5;

/// 最大允许的目录深度（防恶意构造数据 / 损坏的 NTFS）。
pub const MAX_PATH_DEPTH: usize = 256;

/// 一条 record 的最小信息（用于路径重建）。
#[derive(Debug, Clone)]
pub(super) struct RecordInfo {
    pub file_name: String,
    pub parent_reference: u64,
}

/// 路径重建错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RebuildError {
    /// parent_ref 链形成循环（如 A→B→A）。
    CycleDetected,
    /// parent_ref 不在 map 中（stale，快照不一致）。
    StaleParent(u64),
    /// 超过 MAX_PATH_DEPTH 层（防御性，正常 NTFS 不会到这）。
    TooDeep,
}

/// 反向重建：对 `target` 这条 record，从自身向上递归到 NTFS 根，返回完整路径。
///
/// 失败时返回 `RebuildError`。调用方按需 emit `EntryError` 或跳过。
pub(super) fn rebuild_path(
    map: &HashMap<u64, RecordInfo>,
    target: u64,
) -> Result<PathBuf, RebuildError> {
    let mut parts: Vec<&str> = Vec::new();
    let mut current = target;
    let mut visited = Vec::with_capacity(16);

    loop {
        // 命中 NTFS 根：拼接 drive letter 由调用方处理（这里返回 parts 拼成的相对路径）
        if current == NTFS_ROOT_FILE_REFERENCE {
            break;
        }

        // cycle 防御
        if visited.contains(&current) {
            return Err(RebuildError::CycleDetected);
        }
        visited.push(current);

        // 深度防御
        if visited.len() > MAX_PATH_DEPTH {
            return Err(RebuildError::TooDeep);
        }

        let Some(info) = map.get(&current) else {
            return Err(RebuildError::StaleParent(current));
        };
        parts.push(&info.file_name);
        current = info.parent_reference;
    }

    // parts 是 [target_name, parent_name, ..., direct_child_of_root]
    // 反转后拼接：[direct_child_of_root, ..., parent_name, target_name]
    let mut path = PathBuf::new();
    for part in parts.iter().rev() {
        path.push(part);
    }
    Ok(path)
}

/// 批量重建：对 `targets` 中每个 FileRef 重建路径。
///
/// 成功的返回 `(FileRef, PathBuf)`，失败的返回 `(FileRef, RebuildError)`。
/// 调用方按需处理失败项（如 emit `EntryError`）。
#[allow(dead_code)]
pub(super) fn rebuild_paths<'a, I>(
    map: &'a HashMap<u64, RecordInfo>,
    targets: I,
) -> Vec<(u64, Result<PathBuf, RebuildError>)>
where
    I: IntoIterator<Item = &'a u64>,
{
    targets
        .into_iter()
        .map(|&target| (target, rebuild_path(map, target)))
        .collect()
}

/// 给重建出的相对路径加上 drive letter 前缀（如 `C:\`）。
pub(super) fn with_drive_prefix(relative: &Path, drive: char) -> PathBuf {
    let mut result = PathBuf::new();
    result.push(format!("{}:\\", drive.to_ascii_uppercase()));
    if let Some(rest) = strip_root_slash(relative) {
        result.push(rest);
    }
    result
}

fn strip_root_slash(path: &Path) -> Option<&Path> {
    let s = path.to_str()?;
    let trimmed = s.trim_start_matches('/').trim_start_matches('\\');
    if trimmed.is_empty() {
        None
    } else {
        Some(Path::new(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(records: &[(u64, &str, u64)]) -> HashMap<u64, RecordInfo> {
        records
            .iter()
            .map(|&(r, name, p)| {
                (
                    r,
                    RecordInfo {
                        file_name: name.to_string(),
                        parent_reference: p,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn rebuilds_simple_chain() {
        // root=5 → A → B → file
        let map = make_map(&[(10, "A", 5), (11, "B", 10), (12, "file.txt", 11)]);
        let path = rebuild_path(&map, 12).unwrap();
        assert_eq!(path, PathBuf::from("A/B/file.txt"));
    }

    #[test]
    fn rebuilds_top_level_file() {
        let map = make_map(&[(10, "file.txt", 5)]);
        let path = rebuild_path(&map, 10).unwrap();
        assert_eq!(path, PathBuf::from("file.txt"));
    }

    #[test]
    fn detects_cycle() {
        // A → B → A（cycle）
        let map = make_map(&[(10, "A", 11), (11, "B", 10)]);
        let result = rebuild_path(&map, 10);
        assert_eq!(result.err(), Some(RebuildError::CycleDetected));
    }

    #[test]
    fn detects_stale_parent() {
        // parent=99 不在 map 中
        let map = make_map(&[(10, "file.txt", 99)]);
        let result = rebuild_path(&map, 10);
        assert_eq!(result.err(), Some(RebuildError::StaleParent(99)));
    }

    #[test]
    fn detects_too_deep() {
        // 构造 300 层深的链
        let mut records = Vec::new();
        for i in 1..300 {
            records.push((1000 + i, format!("d{i}"), 1000 + i - 1));
        }
        let map: HashMap<u64, RecordInfo> = records
            .iter()
            .map(|(r, n, p)| {
                (
                    *r,
                    RecordInfo {
                        file_name: n.clone(),
                        parent_reference: *p,
                    },
                )
            })
            .collect();
        // 起点 1000 的 parent = 999，不在 map → 应该是 StaleParent，先加 1000 自己
        let mut map = map;
        map.insert(
            1000,
            RecordInfo {
                file_name: "d0".to_string(),
                parent_reference: 5,
            },
        );
        let top = 1000 + 299;
        let result = rebuild_path(&map, top);
        // 256 层限制 vs 299 实际深度 → TooDeep
        assert_eq!(result.err(), Some(RebuildError::TooDeep));
    }

    #[test]
    fn rebuilds_batch() {
        let map = make_map(&[(10, "A", 5), (11, "file1.txt", 10), (12, "file2.txt", 10)]);
        let targets = vec![11u64, 12];
        let results = rebuild_paths(&map, &targets);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
    }

    #[test]
    fn with_drive_prefix_prepends_c() {
        // 在 Windows 上 PathBuf 不会自动 normalize 分隔符，所以比较时统一成 lowercase 后再 normalize
        let rel = PathBuf::from("Windows\\System32\\notepad.exe");
        let full = with_drive_prefix(&rel, 'C');
        let got: String = full
            .to_string_lossy()
            .to_ascii_lowercase()
            .replace('/', "\\");
        assert_eq!(got, "c:\\windows\\system32\\notepad.exe");
    }

    #[test]
    fn with_drive_prefix_handles_root_only() {
        let rel = PathBuf::new();
        let full = with_drive_prefix(&rel, 'D');
        assert_eq!(full.to_str().unwrap(), "D:\\");
    }
}
