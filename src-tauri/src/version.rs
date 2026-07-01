use std::cmp::Ordering;

/// 判断本地版本是否需要更新到目标版本。
///
/// 支持三类版本号格式：
/// - 纯数字版本（`1.2.3`、`v2.0.0`）：按数字段逐段比较。
/// - 预发布版本（`1.2.3-beta`、`1.2.3-rc.1`）：先比 core 数字段，core 相同时
///   按 semver 规则比较预发布标识符（正式版 > 预发布版；数字标识符按数值比较，
///   字母标识符按 ASCII 比较，数字 < 字母）。
/// - 无法解析的版本（`nightly-2026-06-30`、`9.9.9-smoke` 等）：fallback 到
///   字符串不等比较，由调用方保证版本号在相同格式下可比。
pub fn is_update_needed(current_version: Option<&str>, latest_version: &str) -> bool {
    let Some(current_version) = current_version else {
        return true;
    };
    match compare_versions(current_version, latest_version) {
        Some(Ordering::Less) => true,
        Some(Ordering::Equal | Ordering::Greater) => false,
        None => current_version != latest_version,
    }
}

/// 比较两个版本号，返回 `Some(Ordering)` 表示可解析比较，`None` 表示无法解析。
fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let left_parsed = parse_version(left)?;
    let right_parsed = parse_version(right)?;
    match compare_numeric_parts(&left_parsed.core, &right_parsed.core) {
        Ordering::Equal => {}
        ordering => return Some(ordering),
    }
    Some(compare_prerelease(&left_parsed.pre, &right_parsed.pre))
}

/// 解析后的版本号：core 数字段 + 可选预发布后缀。
struct ParsedVersion {
    core: Vec<u64>,
    pre: Option<String>,
}

/// 解析版本号为 core 数字段 + 可选预发布后缀。
///
/// - `1.2.3` → core=[1,2,3], pre=None
/// - `1.2.3-beta` → core=[1,2,3], pre=Some("beta")
/// - `1.2.3-rc.1` → core=[1,2,3], pre=Some("rc.1")
/// - `nightly-2026-06-30` → core 段 "nightly" 非数字 → None（走 fallback 字符串比较）
fn parse_version(version: &str) -> Option<ParsedVersion> {
    // 与 github_release::normalize_release_version 对齐，同时去小写 'v' 和大写 'V' 前缀，
    // 避免大写 V 前缀的版本号（如 "V1.2.3"）走 fallback 字符串比较。
    let trimmed = version.trim().trim_start_matches(['v', 'V']);
    if trimmed.is_empty() {
        return None;
    }
    // 剥离 semver 构建元数据（`+` 及之后内容），它不参与版本比较。
    // 例如 `1.2.3+build.123` → `1.2.3`，`1.2.3-rc.1+build.123` → `1.2.3-rc.1`。
    let trimmed = trimmed
        .split_once('+')
        .map(|(prefix, _)| prefix)
        .unwrap_or(trimmed);
    let (core_str, pre) = match trimmed.split_once('-') {
        Some((core, pre)) => (core, Some(pre.to_string())),
        None => (trimmed, None),
    };
    let core: Vec<u64> = core_str
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    Some(ParsedVersion { core, pre })
}

/// 逐段比较 core 数字段，缺失的段按 0 处理。
fn compare_numeric_parts(left: &[u64], right: &[u64]) -> Ordering {
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let left_part = *left.get(index).unwrap_or(&0);
        let right_part = *right.get(index).unwrap_or(&0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

/// 按 semver 规则比较预发布后缀。
///
/// - 都无 pre：Equal
/// - 一方有 pre：无 pre 的更大（正式版 > 预发布版）
/// - 都有 pre：按标识符逐个比较（数字 < 字母；数字按数值，字母按 ASCII）
fn compare_prerelease(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(l), Some(r)) => compare_prerelease_identifiers(l, r),
    }
}

/// 按 semver 规则逐个比较预发布标识符。
///
/// 标识符以 `.` 分割。数字标识符（全 ASCII 数字）按数值比较，非数字标识符按 ASCII
/// 比较，数字标识符小于字母标识符。标识符数量少的一方更小（`rc.1` < `rc.1.1`）。
fn compare_prerelease_identifiers(left: &str, right: &str) -> Ordering {
    let left_ids: Vec<&str> = left.split('.').collect();
    let right_ids: Vec<&str> = right.split('.').collect();
    let max_len = left_ids.len().max(right_ids.len());
    for index in 0..max_len {
        let Some(l) = left_ids.get(index) else {
            return Ordering::Less;
        };
        let Some(r) = right_ids.get(index) else {
            return Ordering::Greater;
        };
        let l_is_num = !l.is_empty() && l.chars().all(|c| c.is_ascii_digit());
        let r_is_num = !r.is_empty() && r.chars().all(|c| c.is_ascii_digit());
        match (l_is_num, r_is_num) {
            (true, true) => {
                // l_is_num/r_is_num 已保证是合法数字，unwrap_or(0) 仅防 u64 overflow，
                // 实际版本号不会出现超过 u64::MAX 的数字标识符。
                let l_num: u64 = l.parse().unwrap_or(0);
                let r_num: u64 = r.parse().unwrap_or(0);
                match l_num.cmp(&r_num) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }
            }
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => match l.cmp(r) {
                Ordering::Equal => {}
                ordering => return ordering,
            },
        }
    }
    Ordering::Equal
}

#[cfg(test)]
#[path = "test/version.rs"]
mod tests;
