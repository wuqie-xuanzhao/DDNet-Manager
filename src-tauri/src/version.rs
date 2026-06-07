use std::cmp::Ordering;

/// 判断本地版本是否需要更新到目标版本。
pub fn is_update_needed(current_version: Option<&str>, latest_version: &str) -> bool {
    let Some(current_version) = current_version else {
        return true;
    };

    match compare_numeric_version(current_version, latest_version) {
        Some(Ordering::Less) => true,
        Some(Ordering::Equal | Ordering::Greater) => false,
        None => current_version != latest_version,
    }
}

fn compare_numeric_version(left: &str, right: &str) -> Option<Ordering> {
    let left_parts = numeric_version_parts(left)?;
    let right_parts = numeric_version_parts(right)?;
    let max_len = left_parts.len().max(right_parts.len());

    for index in 0..max_len {
        let left_part = *left_parts.get(index).unwrap_or(&0);
        let right_part = *right_parts.get(index).unwrap_or(&0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return Some(ordering),
        }
    }

    Some(Ordering::Equal)
}

fn numeric_version_parts(version: &str) -> Option<Vec<u64>> {
    let trimmed = version.trim().trim_start_matches('v');
    if trimmed.is_empty() {
        return None;
    }

    trimmed
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
#[path = "test/version.rs"]
mod tests;
