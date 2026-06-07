use std::path::{Path, PathBuf};

/// DDNet Manager 管理区块起始标记。
pub const MANAGER_BEGIN: &str = "# DDNET_MANAGER_BEGIN";

/// DDNet Manager 管理区块结束标记。
pub const MANAGER_END: &str = "# DDNET_MANAGER_END";

/// 渲染仅由 DDNet Manager 管理的 cfg 文本区块。
pub fn render_manager_cfg(commands: &[String]) -> String {
    let mut output = String::new();
    output.push_str(MANAGER_BEGIN);
    output.push('\n');
    output.push_str("# This block is managed by DDNet Manager.\n");
    for command in commands {
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        output.push_str(command);
        output.push('\n');
    }
    output.push_str(MANAGER_END);
    output.push('\n');
    output
}

/// 校验 Manager 管理区块内的命令文本，拒绝破坏区块结构的输入。
pub fn validate_manager_commands(commands: &[String]) -> Result<(), String> {
    for command in commands {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.contains('\n') || trimmed.contains('\r') {
            return Err("manager command must not contain newline".to_string());
        }

        if trimmed.contains(MANAGER_BEGIN) || trimmed.contains(MANAGER_END) {
            return Err("manager command must not contain manager markers".to_string());
        }
    }

    Ok(())
}

/// 在原文件同目录创建一个带时间戳后缀的备份副本。
pub fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let backup_path = next_backup_path(path)?;

    std::fs::copy(path, &backup_path)
        .map_err(|error| format!("failed to backup {}: {error}", path.display()))?;

    Ok(backup_path)
}

fn next_backup_path(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid file name: {}", path.display()))?;
    let timestamp = chrono_like_timestamp();
    let base_name = format!("{file_name}.bak.{timestamp}");
    let first_candidate = path.with_file_name(&base_name);

    if !first_candidate.exists() {
        return Ok(first_candidate);
    }

    let mut suffix = 1_u32;
    loop {
        let candidate = path.with_file_name(format!("{base_name}.{suffix}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
        suffix = suffix.saturating_add(1);
    }
}

fn chrono_like_timestamp() -> String {
    let duration = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration,
        Err(_) => return "0".to_string(),
    };

    duration.as_millis().to_string()
}

#[cfg(test)]
#[path = "test/file_tx.rs"]
mod tests;
