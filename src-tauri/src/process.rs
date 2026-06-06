use crate::models::ClientHealth;
use std::path::{Path, PathBuf};

/// 表示通过安全校验后的客户端启动目标。
pub struct LaunchTarget {
    /// 客户端可执行文件路径。
    pub executable_path: PathBuf,
    /// 客户端启动工作目录。
    pub working_dir: PathBuf,
}

/// 判断进程名是否为 DDNet 客户端可执行文件名。
pub fn is_ddnet_process_name(name: &str) -> bool {
    matches!(name, "DDNet.exe" | "ddnet.exe" | "DDNet" | "ddnet")
}

/// 解析并校验客户端启动目标，确保只启动完整客户端目录内的 DDNet 可执行文件。
pub fn resolve_launch_target(path: &Path) -> Result<LaunchTarget, String> {
    if !path.is_file() {
        return Err(format!("launch target is not a file: {}", path.display()));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("launch target has invalid file name: {}", path.display()))?;

    if !is_ddnet_process_name(file_name) {
        return Err(format!(
            "launch target is not a DDNet executable: {file_name}"
        ));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("launch target has no parent directory: {}", path.display()))?;

    if !parent.is_dir() {
        return Err(format!(
            "launch target parent is not a directory: {}",
            parent.display()
        ));
    }

    let installation = crate::client_scan::validate_client_dir(parent)?;
    if installation.health != ClientHealth::Ok {
        return Err(format!(
            "launch target client directory is not healthy: {:?}",
            installation.health
        ));
    }

    Ok(LaunchTarget {
        executable_path: path.to_path_buf(),
        working_dir: parent.to_path_buf(),
    })
}

/// 启动指定路径的客户端可执行文件。
pub fn launch_executable(path: &str) -> Result<(), String> {
    let target = resolve_launch_target(Path::new(path))?;

    std::process::Command::new(&target.executable_path)
        .current_dir(&target.working_dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch {path}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn recognizes_ddnet_process_names() {
        assert!(super::is_ddnet_process_name("DDNet.exe"));
        assert!(super::is_ddnet_process_name("ddnet.exe"));
        assert!(super::is_ddnet_process_name("DDNet"));
        assert!(super::is_ddnet_process_name("ddnet"));
    }

    #[test]
    fn rejects_non_ddnet_process_names() {
        assert!(!super::is_ddnet_process_name("notepad.exe"));
    }

    #[test]
    fn resolve_launch_target_rejects_non_ddnet_file_name() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("notepad.exe"), b"").expect("测试文件应写入成功");

        let result = super::resolve_launch_target(&install_dir.join("notepad.exe"));

        assert!(result.is_err());
    }

    #[test]
    fn resolve_launch_target_rejects_missing_file() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();

        let result = super::resolve_launch_target(&install_dir.join("DDNet.exe"));

        assert!(result.is_err());
    }

    #[test]
    fn resolve_launch_target_rejects_incomplete_client_directory() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");

        let result = super::resolve_launch_target(&install_dir.join("DDNet.exe"));

        assert!(result.is_err());
    }

    #[test]
    fn resolve_launch_target_accepts_complete_client_directory() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        let executable_path = install_dir.join("DDNet.exe");
        fs::write(&executable_path, b"").expect("测试可执行文件应写入成功");
        fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let target = super::resolve_launch_target(&executable_path).expect("完整客户端应可解析");

        assert_eq!(target.executable_path, executable_path);
        assert_eq!(target.working_dir, install_dir);
    }
}
