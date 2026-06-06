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
mod tests {
    use super::*;

    #[test]
    fn renders_manager_cfg_with_exact_structure() {
        let output = render_manager_cfg(&[
            "bind mouse3 \"echo Kill; kill\"".to_string(),
            "bind ctrl+z \"echo 自救; say /rescue\"".to_string(),
        ]);

        assert_eq!(
            output,
            "# DDNET_MANAGER_BEGIN\n# This block is managed by DDNet Manager.\nbind mouse3 \"echo Kill; kill\"\nbind ctrl+z \"echo 自救; say /rescue\"\n# DDNET_MANAGER_END\n"
        );
    }

    #[test]
    fn trims_commands_before_rendering() {
        let output = render_manager_cfg(&["  bind f1 \"say hi\"  ".to_string()]);

        assert_eq!(
            output,
            "# DDNET_MANAGER_BEGIN\n# This block is managed by DDNet Manager.\nbind f1 \"say hi\"\n# DDNET_MANAGER_END\n"
        );
    }

    #[test]
    fn skips_empty_or_whitespace_only_commands() {
        let output = render_manager_cfg(&[
            "".to_string(),
            "   ".to_string(),
            "\t".to_string(),
            " bind mouse5 \"toggle cl_showfps 0 1\" ".to_string(),
        ]);

        assert_eq!(
            output,
            "# DDNET_MANAGER_BEGIN\n# This block is managed by DDNet Manager.\nbind mouse5 \"toggle cl_showfps 0 1\"\n# DDNET_MANAGER_END\n"
        );
    }

    #[test]
    fn renders_fixed_structure_for_empty_commands() {
        let output = render_manager_cfg(&[]);

        assert_eq!(
            output,
            "# DDNET_MANAGER_BEGIN\n# This block is managed by DDNet Manager.\n# DDNET_MANAGER_END\n"
        );
    }

    #[test]
    fn backup_file_creates_copy_next_to_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cfg_path = temp.path().join("settings_ddnet.cfg");
        std::fs::write(&cfg_path, "bind mouse3 \"echo Kill; kill\"\n").expect("write");

        let backup = backup_file(&cfg_path).expect("backup");

        assert!(backup.is_file());
        assert!(backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(".bak."));
    }

    #[test]
    fn backup_file_preserves_original_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cfg_path = temp.path().join("settings_ddnet.cfg");
        let original = "bind mouse3 \"echo Kill; kill\"\n";
        std::fs::write(&cfg_path, original).expect("write");

        let backup = backup_file(&cfg_path).expect("backup");
        let backup_content = std::fs::read_to_string(backup).expect("read backup");

        assert_eq!(backup_content, original);
    }

    #[test]
    fn backup_file_skips_existing_candidate_path_without_overwriting() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cfg_path = temp.path().join("settings_ddnet.cfg");
        std::fs::write(&cfg_path, "bind mouse3 \"echo Kill; kill\"\n").expect("write");

        let occupied = next_backup_path(&cfg_path).expect("occupied path");
        std::fs::write(&occupied, "existing backup").expect("write occupied");

        let backup = backup_file(&cfg_path).expect("backup");

        assert_ne!(backup, occupied);
        assert_eq!(
            std::fs::read_to_string(&occupied).expect("read occupied"),
            "existing backup"
        );
        assert_eq!(
            std::fs::read_to_string(&backup).expect("read backup"),
            "bind mouse3 \"echo Kill; kill\"\n"
        );
    }

    #[test]
    fn validate_manager_commands_rejects_multiline_command() {
        let error =
            validate_manager_commands(&["bind f1 \"say hi\"\nbind f2 \"say bye\"".to_string()])
                .expect_err("multiline should fail");

        assert!(error.contains("must not contain newline"));
    }

    #[test]
    fn validate_manager_commands_rejects_marker_injection() {
        let error = validate_manager_commands(&[format!("bind f1 \"echo {}\"", MANAGER_END)])
            .expect_err("marker should fail");

        assert!(error.contains("must not contain manager markers"));
    }

    #[test]
    fn validate_manager_commands_allows_normal_commands() {
        let commands = vec![
            " bind f1 \"say hi\" ".to_string(),
            "bind mouse5 \"toggle cl_showfps 0 1\"".to_string(),
        ];

        assert!(validate_manager_commands(&commands).is_ok());
    }
}
