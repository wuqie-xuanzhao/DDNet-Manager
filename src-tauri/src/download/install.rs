use crate::error::ManagerError;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 将 staging 中的客户端安装到目标目录，并为旧安装创建回滚目录。
pub fn install_staged_client(
    staged_client_dir: &Path,
    install_dir: &Path,
    rollback_dir: &Path,
) -> Result<(), ManagerError> {
    let replacement_dir = replacement_dir_for(install_dir);
    if replacement_dir.exists() {
        fs::remove_dir_all(&replacement_dir).map_err(|error| {
            ManagerError::Internal(format!("failed to clear replacement dir: {error}"))
        })?;
    }
    if rollback_dir.exists() {
        fs::remove_dir_all(rollback_dir).map_err(|error| {
            ManagerError::Internal(format!("failed to clear rollback dir: {error}"))
        })?;
    }
    if let Some(parent) = rollback_dir.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ManagerError::Internal(format!("failed to create rollback parent: {error}"))
        })?;
    }

    copy_dir_recursive(staged_client_dir, &replacement_dir)?;
    let replacement_client = crate::client_scan::validate_client_dir(&replacement_dir)
        .map_err(|error| ManagerError::Internal(error.to_string()))?;
    if replacement_client.health != crate::models::ClientHealth::Ok {
        let _ = fs::remove_dir_all(&replacement_dir);
        return Err(ManagerError::Internal(format!(
            "replacement client is not healthy: {:?}",
            replacement_client.health
        )));
    }

    let had_existing_install = install_dir.exists();
    if had_existing_install {
        // Windows 上若 install_dir 仍被进程占用（典型场景：安装中段用户重新启动了
        // DDNet），rename 会以 AccessDenied 失败。这里给出明确诊断，避免上层把
        // 这种可恢复错误归类为内部错误。
        if let Err(error) = fs::rename(install_dir, rollback_dir) {
            let is_busy = crate::process::is_install_dir_busy(install_dir);
            let message = if is_busy {
                format!("target install dir is busy; close the running client and retry: {error}")
            } else {
                format!("failed to create rollback point: {error}")
            };
            return Err(if is_busy {
                ManagerError::ClientRunning(message)
            } else {
                ManagerError::Internal(message)
            });
        }
    }

    if let Err(error) = fs::rename(&replacement_dir, install_dir) {
        if had_existing_install && rollback_dir.exists() {
            if let Err(restore_error) = fs::rename(rollback_dir, install_dir) {
                // 双失败：原 install_dir 已被改名为 rollback_dir，激活与恢复都失败，
                // rollback_dir 仍保留在磁盘上。错误信息显式带上 rollback_dir 路径，
                // 让用户/运维知道从哪里手动恢复旧版本。
                return Err(ManagerError::Internal(format!(
                    "failed to activate replacement: {error}; failed to restore rollback: {restore_error}; \
                     rollback_dir={} (recover manually if needed)",
                    rollback_dir.display()
                )));
            }
        }
        return Err(ManagerError::Internal(format!(
            "failed to activate replacement: {error}"
        )));
    }

    Ok(())
}

/// 返回位于安装目录同级的回滚目录，避免 Windows 跨盘 rename 失败。
pub fn rollback_dir_for(install_dir: &Path, install_id: &str) -> PathBuf {
    let name = install_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ddnet-client");
    install_dir.with_file_name(format!("{name}.ddnet-manager-rollback-{install_id}"))
}

/// 使用已创建的回滚目录恢复目标安装目录。
pub fn restore_rollback(install_dir: &Path, rollback_dir: &Path) -> Result<(), ManagerError> {
    if !rollback_dir.exists() {
        return Err(ManagerError::NotFound(format!(
            "rollback dir does not exist: {}",
            rollback_dir.display()
        )));
    }

    let failed_dir = failed_restore_dir_for(install_dir);
    if failed_dir.exists() {
        fs::remove_dir_all(&failed_dir).map_err(|error| {
            ManagerError::Internal(format!("failed to clear failed restore dir: {error}"))
        })?;
    }

    let had_active_install = install_dir.exists();
    if had_active_install {
        fs::rename(install_dir, &failed_dir).map_err(|error| {
            ManagerError::Internal(format!(
                "failed to move active install before rollback: {error}"
            ))
        })?;
    }

    if let Err(error) = fs::rename(rollback_dir, install_dir) {
        if had_active_install && failed_dir.exists() {
            if let Err(restore_error) = fs::rename(&failed_dir, install_dir) {
                return Err(ManagerError::Internal(format!(
                    "failed to restore rollback: {error}; failed to restore active install: {restore_error}"
                )));
            }
        }
        return Err(ManagerError::Internal(format!(
            "failed to restore rollback: {error}"
        )));
    }

    if failed_dir.exists() {
        fs::remove_dir_all(&failed_dir).map_err(|error| {
            ManagerError::Internal(format!(
                "failed to clear replaced install after rollback: {error}"
            ))
        })?;
    }

    Ok(())
}

/// 判断目录项名称是否为本管理器产生的安装残留产物。
///
/// 仅匹配三类后缀模式，避免 `contains` 模糊匹配误伤用户在客户端父目录下
/// 自建的合法目录（例如 `ddnet-manager-rollback-tutorial`、
/// `my.ddnet-manager-rollback.backup` 等）。
///
/// 识别规则：
/// - marker 必须以 dot 开头，且 marker 之前必须有非空 prefix
/// - rollback 模式的 suffix（即 install_id）至少 4 字符且不含 dot，
///   因为 install_id 形如 `install-<uuid>`，而合法目录常有 `.backup` 等
///   扩展名 —— 这条规则用来排除 `my.ddnet-manager-rollback-notes.backup` 这类
///   形似但非本管理器产生的目录
/// - replacement / restore-failed 模式必须出现在文件名末尾
fn is_install_artifact_name(name: &str) -> bool {
    const ROLLBACK_MARKER: &str = ".ddnet-manager-rollback-";
    const REPLACEMENT_MARKER: &str = ".ddnet-manager-replacement";
    const RESTORE_FAILED_MARKER: &str = ".ddnet-manager-restore-failed";

    // rollback: <prefix>.ddnet-manager-rollback-<install_id>
    //   prefix 非空，install_id 非空、不含 dot、长度 >= 4
    if let Some(idx) = name.find(ROLLBACK_MARKER) {
        let prefix = &name[..idx];
        let suffix = &name[idx + ROLLBACK_MARKER.len()..];
        if !prefix.is_empty() && suffix.len() >= 4 && !suffix.contains('.') {
            return true;
        }
    }

    // replacement: <prefix>.ddnet-manager-replacement 或 <prefix>.ddnet-manager-replacement.app
    //   marker 必须出现在末尾（可选 .app 扩展名）。
    if let Some(idx) = name.find(REPLACEMENT_MARKER) {
        let prefix = &name[..idx];
        let suffix = &name[idx + REPLACEMENT_MARKER.len()..];
        if !prefix.is_empty() && (suffix.is_empty() || suffix == ".app") {
            return true;
        }
    }

    // restore-failed: <prefix>.ddnet-manager-restore-failed
    if let Some(idx) = name.find(RESTORE_FAILED_MARKER) {
        let prefix = &name[..idx];
        let suffix = &name[idx + RESTORE_FAILED_MARKER.len()..];
        if !prefix.is_empty() && suffix.is_empty() {
            return true;
        }
    }

    false
}

/// 清理指定目录下所有由管理器创建的回滚、替换和恢复失败残留目录。
///
/// 仅匹配三类精确后缀模式：
/// - `<name>.ddnet-manager-rollback-<install_id>`
/// - `<name>.ddnet-manager-replacement[.app]`
/// - `<name>.ddnet-manager-restore-failed`
///
/// `protected_paths` 中的路径会被跳过，用于保留成功安装后留下的可回滚 rollback 目录。
/// 调用方应从 `install_history` 表查询所有 `Completed` 记录的 `rollback_path` 组装此集合。
/// 路径比较统一规范化为正斜杠字符串，兼容 Windows 反斜杠与 history 表中已规范化的存储。
///
/// 返回已清理的目录数量。
pub fn cleanup_stale_install_artifacts(
    scan_dir: &Path,
    protected_paths: &HashSet<PathBuf>,
) -> Result<usize, ManagerError> {
    if !scan_dir.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(scan_dir).map_err(|error| {
        ManagerError::Internal(format!("failed to read dir for cleanup: {error}"))
    })?;
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ManagerError::Internal(format!("failed to read cleanup entry: {error}"))
        })?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let entry_path = entry.path();
        if !is_install_artifact_name(&name_str) || !entry_path.is_dir() {
            continue;
        }
        if path_is_protected(&entry_path, protected_paths) {
            continue;
        }
        fs::remove_dir_all(&entry_path).map_err(|error| {
            ManagerError::Internal(format!(
                "failed to remove stale artifact '{}': {error}",
                name_str
            ))
        })?;
        removed += 1;
    }
    Ok(removed)
}

/// 判断扫描到的目录路径是否在受保护集合中。
///
/// 两边都规范化为正斜杠字符串后比较，兼容 Windows 反斜杠与 history 表中已规范化的存储。
fn path_is_protected(path: &Path, protected: &HashSet<PathBuf>) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    protected
        .iter()
        .any(|p| p.to_string_lossy().replace('\\', "/") == normalized)
}

/// 启动时清理上次进程崩溃留下的 staging 残留。
///
/// staging_root 通常是 `<app_cache>/staging`，每次安装事务在它下面建
/// `install-<job_id>/` 子目录（见 [`crate::commands::install`]）。崩溃前若已解压
/// 但未完成 install，残留会逐步累积磁盘占用。启动时无活跃安装任务，所有
/// `install-*` 子目录都可安全删除。
///
/// 仅清理 `install-` 前缀的子目录，保留 staging_root 本身与其他名字的目录。
pub fn cleanup_stale_staging(staging_root: &Path) -> Result<usize, ManagerError> {
    if !staging_root.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(staging_root)
        .map_err(|error| ManagerError::Internal(format!("failed to read staging root: {error}")))?;
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ManagerError::Internal(format!("failed to read staging entry: {error}"))
        })?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path_starts_with_install(&entry.path(), &name_str) {
            fs::remove_dir_all(entry.path()).map_err(|error| {
                ManagerError::Internal(format!(
                    "failed to remove stale staging '{}': {error}",
                    name_str
                ))
            })?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn path_starts_with_install(path: &Path, name: &str) -> bool {
    path.is_dir() && name.starts_with("install-")
}

fn replacement_dir_for(install_dir: &Path) -> PathBuf {
    let name = install_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ddnet-client");
    if install_dir
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    {
        let stem = install_dir
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("ddnet-client");
        install_dir.with_file_name(format!("{stem}.ddnet-manager-replacement.app"))
    } else {
        install_dir.with_file_name(format!("{name}.ddnet-manager-replacement"))
    }
}

fn failed_restore_dir_for(install_dir: &Path) -> PathBuf {
    let name = install_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ddnet-client");
    install_dir.with_file_name(format!("{name}.ddnet-manager-restore-failed"))
}

/// 递归复制目录树，保留常规文件、目录与 symlink；用于把 staging 客户端拷贝到
/// replacement 目录或从 dmg 镜像拷出 app bundle。
pub(crate) fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), ManagerError> {
    fs::create_dir_all(destination).map_err(|error| {
        ManagerError::Internal(format!("failed to create install dir: {error}"))
    })?;

    for entry in fs::read_dir(source)
        .map_err(|error| ManagerError::Internal(format!("failed to read source dir: {error}")))?
    {
        let entry = entry.map_err(|error| {
            ManagerError::Internal(format!("failed to read source entry: {error}"))
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| {
            ManagerError::Internal(format!("failed to read source file type: {error}"))
        })?;
        if file_type.is_symlink() {
            copy_symlink(&source_path, &destination_path)?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                ManagerError::Internal(format!("failed to copy install file: {error}"))
            })?;
        }
    }

    Ok(())
}

fn copy_symlink(source_path: &Path, destination_path: &Path) -> Result<(), ManagerError> {
    #[cfg(unix)]
    {
        let target = fs::read_link(source_path).map_err(|error| {
            ManagerError::Internal(format!("failed to read install symlink: {error}"))
        })?;
        std::os::unix::fs::symlink(&target, destination_path).map_err(|error| {
            ManagerError::Internal(format!("failed to copy install symlink: {error}"))
        })
    }

    #[cfg(windows)]
    {
        let target = fs::read_link(source_path).map_err(|error| {
            ManagerError::Internal(format!("failed to read install symlink: {error}"))
        })?;
        if source_path.is_dir() {
            std::os::windows::fs::symlink_dir(&target, destination_path).map_err(|error| {
                ManagerError::Internal(format!("failed to copy install symlink: {error}"))
            })
        } else {
            std::os::windows::fs::symlink_file(&target, destination_path).map_err(|error| {
                ManagerError::Internal(format!("failed to copy install symlink: {error}"))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn empty_protected() -> HashSet<PathBuf> {
        HashSet::new()
    }

    #[test]
    fn cleanup_stale_install_artifacts_removes_matching_dirs() {
        let base = tempfile::tempdir().expect("temp dir");
        // 使用贴近真实命名的 install_id（install-<uuid> 形式），让精确后缀匹配生效。
        let rollback = base
            .path()
            .join("client.ddnet-manager-rollback-install-abc");
        let replacement = base.path().join("client.ddnet-manager-replacement");
        let restore_failed = base.path().join("client.ddnet-manager-restore-failed");
        let legit = base.path().join("client-data");
        fs::create_dir_all(&rollback).expect("rollback");
        fs::create_dir_all(&replacement).expect("replacement");
        fs::create_dir_all(&restore_failed).expect("restore-failed");
        fs::create_dir_all(&legit).expect("legit");

        let removed =
            cleanup_stale_install_artifacts(base.path(), &empty_protected()).expect("cleanup");
        assert_eq!(removed, 3);
        assert!(!rollback.exists());
        assert!(!replacement.exists());
        assert!(!restore_failed.exists());
        assert!(legit.exists());
    }

    #[test]
    fn cleanup_stale_install_artifacts_recognizes_app_replacement_variant() {
        // macOS .app 目录有自己的扩展名，replacement 命名为 .ddnet-manager-replacement.app
        let base = tempfile::tempdir().expect("temp dir");
        let replacement_app = base.path().join("DDNet.ddnet-manager-replacement.app");
        fs::create_dir_all(&replacement_app).expect("replacement.app");

        let removed =
            cleanup_stale_install_artifacts(base.path(), &empty_protected()).expect("cleanup");
        assert_eq!(removed, 1);
        assert!(!replacement_app.exists());
    }

    #[test]
    fn cleanup_stale_install_artifacts_skips_non_dir_entries() {
        let base = tempfile::tempdir().expect("temp dir");
        // 名称匹配 artifact 模式但本身是文件而非目录，cleanup 应跳过不删。
        let file_path = base
            .path()
            .join("client.ddnet-manager-rollback-install-xyz");
        fs::write(&file_path, "not a dir").expect("file");
        let removed =
            cleanup_stale_install_artifacts(base.path(), &empty_protected()).expect("cleanup");
        assert_eq!(removed, 0);
        assert!(file_path.exists());
    }

    #[test]
    fn cleanup_stale_install_artifacts_returns_zero_for_missing_dir() {
        let missing = PathBuf::from("C:\\nonexistent-ddnet-test-path-12345");
        let removed =
            cleanup_stale_install_artifacts(&missing, &empty_protected()).expect("cleanup");
        assert_eq!(removed, 0);
    }

    #[test]
    fn cleanup_stale_install_artifacts_skips_lookalike_user_dirs() {
        let base = tempfile::tempdir().expect("temp dir");
        // 用户的合法目录名仅"包含"残留模式字符串，但不是后缀——不应被误删。
        let lookalike_rollback_notes = base.path().join("my.ddnet-manager-rollback-notes.backup");
        let lookalike_replaced_dir = base.path().join("ddnet-manager-replacement-tutorial");
        let lookalike_prefix_only = base.path().join(".ddnet-manager-rollback-"); // 后缀为空，视为非法
        fs::create_dir_all(&lookalike_rollback_notes).expect("lookalike notes");
        fs::create_dir_all(&lookalike_replaced_dir).expect("lookalike tutorial");
        fs::create_dir_all(&lookalike_prefix_only).expect("empty suffix");

        let removed =
            cleanup_stale_install_artifacts(base.path(), &empty_protected()).expect("cleanup");
        assert_eq!(removed, 0, "no lookalike user dir should be removed");
        assert!(lookalike_rollback_notes.exists());
        assert!(lookalike_replaced_dir.exists());
        assert!(lookalike_prefix_only.exists());
    }

    #[test]
    fn cleanup_stale_install_artifacts_preserves_protected_rollback_dirs() {
        // 守卫：成功安装后保留的 rollback 目录不应被 cleanup 清掉，
        // 否则用户主动 rollback IPC 重启后失效（缺口3 核心）。
        let base = tempfile::tempdir().expect("temp dir");
        let rollback_kept = base
            .path()
            .join("client.ddnet-manager-rollback-install-keep");
        let rollback_stale = base
            .path()
            .join("client.ddnet-manager-rollback-install-stale");
        let replacement = base.path().join("client.ddnet-manager-replacement");
        fs::create_dir_all(&rollback_kept).expect("rollback-keep");
        fs::create_dir_all(&rollback_stale).expect("rollback-stale");
        fs::create_dir_all(&replacement).expect("replacement");

        let mut protected = HashSet::new();
        protected.insert(rollback_kept.clone());

        let removed = cleanup_stale_install_artifacts(base.path(), &protected).expect("cleanup");
        assert_eq!(removed, 2, "stale rollback + replacement 应被清理");
        assert!(
            rollback_kept.exists(),
            "protected rollback 必须保留供 IPC rollback 使用"
        );
        assert!(!rollback_stale.exists());
        assert!(!replacement.exists());
    }

    #[test]
    fn cleanup_stale_install_artifacts_protected_path_normalizes_backslash() {
        // history 表中 rollback_path 存储为正斜杠（install_history_record 用 replace 规范化），
        // 但扫描时 entry.path() 在 Windows 上是反斜杠。两边都必须规范化后比较。
        let base = tempfile::tempdir().expect("temp dir");
        let rollback = base
            .path()
            .join("client.ddnet-manager-rollback-install-norm");
        fs::create_dir_all(&rollback).expect("rollback");

        // 模拟 history 表中的正斜杠存储
        let protected_str = rollback.to_string_lossy().replace('\\', "/");
        let mut protected = HashSet::new();
        protected.insert(PathBuf::from(protected_str));

        let removed = cleanup_stale_install_artifacts(base.path(), &protected).expect("cleanup");
        assert_eq!(
            removed, 0,
            "正斜杠形式 protected 路径应能匹配反斜杠扫描结果"
        );
        assert!(rollback.exists());
    }

    #[test]
    fn cleanup_stale_staging_removes_install_prefix_dirs() {
        let base = tempfile::tempdir().expect("temp dir");
        let staging_root = base.path().join("staging");
        let install_a = staging_root.join("install-abc");
        let install_b = staging_root.join("install-def");
        let user_data = staging_root.join("user-data"); // 非 install- 前缀，保留
        fs::create_dir_all(&install_a).expect("install-a");
        fs::create_dir_all(&install_b).expect("install-b");
        fs::create_dir_all(&user_data).expect("user-data");

        let removed = cleanup_stale_staging(&staging_root).expect("cleanup");

        assert_eq!(removed, 2);
        assert!(!install_a.exists());
        assert!(!install_b.exists());
        assert!(user_data.exists(), "非 install- 前缀目录应保留");
        assert!(staging_root.exists(), "staging 根目录本身应保留");
    }

    #[test]
    fn cleanup_stale_staging_skips_lookalike_files() {
        let base = tempfile::tempdir().expect("temp dir");
        let staging_root = base.path().join("staging");
        // 名字以 install- 开头但本身是文件，cleanup 应跳过。
        let file_path = staging_root.join("install-not-a-dir");
        fs::create_dir_all(&staging_root).expect("staging root");
        fs::write(&file_path, "marker").expect("file");

        let removed = cleanup_stale_staging(&staging_root).expect("cleanup");

        assert_eq!(removed, 0);
        assert!(file_path.exists());
    }

    #[test]
    fn cleanup_stale_staging_returns_zero_for_missing_dir() {
        let missing = PathBuf::from("C:\\nonexistent-ddnet-test-staging-12345");
        let removed = cleanup_stale_staging(&missing).expect("cleanup");
        assert_eq!(removed, 0);
    }
}
