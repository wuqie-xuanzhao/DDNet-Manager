use crate::models::ClientHealth;
use crate::registry::LaunchProbeStatus;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 表示通过安全校验后的客户端启动目标。
pub struct LaunchTarget {
    /// 客户端可执行文件路径。
    pub executable_path: PathBuf,
    /// 客户端启动工作目录。
    pub working_dir: PathBuf,
}

/// 表示一次受控启动探测的结果。
pub struct LaunchProbeResult {
    /// 启动探测状态。
    pub status: LaunchProbeStatus,
    /// 可直接写入兼容性记录的结果摘要。
    pub message: String,
}

/// 判断进程名是否为 DDNet 客户端可执行文件名。
pub fn is_ddnet_process_name(name: &str) -> bool {
    matches!(name, "DDNet.exe" | "ddnet.exe" | "DDNet" | "ddnet")
}

/// 判断系统中是否存在正在运行的指定客户端可执行文件。
pub fn is_client_running(path: &Path) -> Result<bool, String> {
    let target = normalize_executable_path(path)?;
    Ok(system_process_matches_path(&target))
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

/// 启动客户端并在限定时间内观察进程是否出现。
pub fn launch_executable_with_probe(
    path: &str,
    timeout: Duration,
) -> Result<LaunchProbeResult, String> {
    let target = resolve_launch_target(Path::new(path))?;
    let observed_target = normalize_executable_path(&target.executable_path)?;
    let mut child = std::process::Command::new(&target.executable_path)
        .current_dir(&target.working_dir)
        .spawn()
        .map_err(|error| format!("failed to launch {path}: {error}"))?;
    let started_at = Instant::now();

    loop {
        if system_process_matches_path(&observed_target) {
            return Ok(LaunchProbeResult {
                status: LaunchProbeStatus::Verified,
                message: "受控启动探测已观察到客户端进程".to_string(),
            });
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to observe launched process: {error}"))?
        {
            return Ok(LaunchProbeResult {
                status: LaunchProbeStatus::Exited,
                message: format!("客户端启动后提前退出: {status}"),
            });
        }

        if started_at.elapsed() >= timeout {
            return Ok(LaunchProbeResult {
                status: LaunchProbeStatus::Unobserved,
                message: "客户端已启动，但未在限定时间内观察到进程".to_string(),
            });
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn normalize_executable_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!(
            "client executable is not a file: {}",
            path.display()
        ));
    }

    std::fs::canonicalize(path)
        .map_err(|error| format!("failed to canonicalize client executable: {error}"))
}

fn system_process_matches_path(target: &Path) -> bool {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    system
        .processes()
        .values()
        .filter_map(|process| process.exe())
        .any(|path| process_path_matches(path, target))
}

fn process_path_matches(process_path: &Path, target: &Path) -> bool {
    std::fs::canonicalize(process_path).is_ok_and(|path| path == target)
}

#[cfg(test)]
#[path = "test/process.rs"]
mod tests;
