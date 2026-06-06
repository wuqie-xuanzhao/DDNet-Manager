use crate::models::{ClientHealth, ClientInstallation};
use std::path::{Path, PathBuf};
use thiserror::Error;

const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A_64_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Error)]
enum ClientScanError {
    #[error("客户端路径不是目录: {0}")]
    NotDirectory(String),
}

/// 验证 DDNet 兼容客户端目录，并返回可供前端展示的安装记录。
pub fn validate_client_dir(path: &Path) -> Result<ClientInstallation, String> {
    if !path.is_dir() {
        return Err(ClientScanError::NotDirectory(normalize_path(path)).to_string());
    }

    let executable_path = path.join("DDNet.exe");
    let storage_cfg_path = path.join("storage.cfg");
    let data_dir = path.join("data");
    let install_dir = normalize_path(path);
    let id_seed = normalized_id_seed(path);

    let health = detect_client_health(&executable_path, &storage_cfg_path, &data_dir);

    Ok(ClientInstallation {
        id: stable_installation_id(&id_seed),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir,
        executable_path: normalize_path(&executable_path),
        storage_cfg_path: normalize_path(&storage_cfg_path),
        data_dir: normalize_path(&data_dir),
        user_data_dir: find_ddnet_user_data_dir(),
        version: None,
        is_default: false,
        health,
    })
}

fn detect_client_health(
    executable_path: &Path,
    storage_cfg_path: &Path,
    data_dir: &Path,
) -> ClientHealth {
    if !executable_path.is_file() {
        return ClientHealth::MissingExecutable;
    }

    if !storage_cfg_path.is_file() {
        return ClientHealth::MissingStorageCfg;
    }

    if !data_dir.is_dir() {
        return ClientHealth::MissingDataDir;
    }

    ClientHealth::Ok
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalized_id_seed(path: &Path) -> String {
    let canonical_path = canonicalize_existing_dir(path);
    normalize_id_seed(&normalize_path(&canonical_path))
}

fn canonicalize_existing_dir(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(windows)]
fn normalize_id_seed(path: &str) -> String {
    path.to_ascii_lowercase()
}

#[cfg(not(windows))]
fn normalize_id_seed(path: &str) -> String {
    path.to_string()
}

fn find_ddnet_user_data_dir() -> Option<String> {
    let user_data_dir = dirs::config_dir()?.join("DDNet");

    if user_data_dir.is_dir() {
        Some(normalize_path(&user_data_dir))
    } else {
        None
    }
}

fn stable_installation_id(path: &str) -> String {
    let hash = path
        .as_bytes()
        .iter()
        .fold(FNV1A_64_OFFSET_BASIS, |hash, byte| {
            let mixed = hash ^ u64::from(*byte);
            mixed.wrapping_mul(FNV1A_64_PRIME)
        });

    format!("qmclient-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{normalize_path, stable_installation_id, validate_client_dir};
    use crate::models::ClientHealth;
    use std::fs;
    use std::path::Path;

    #[test]
    fn validate_client_dir_returns_ok_for_complete_directory() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = validate_client_dir(install_dir).expect("完整目录应验证成功");

        assert_eq!(installation.health, ClientHealth::Ok);
        assert!(installation.executable_path.ends_with("DDNet.exe"));
    }

    #[test]
    fn validate_client_dir_reports_missing_executable() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation =
            validate_client_dir(install_dir).expect("缺少可执行文件也应返回安装记录");

        assert_eq!(installation.health, ClientHealth::MissingExecutable);
    }

    #[test]
    fn validate_client_dir_reports_missing_storage_cfg() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation =
            validate_client_dir(install_dir).expect("缺少 storage.cfg 也应返回安装记录");

        assert_eq!(installation.health, ClientHealth::MissingStorageCfg);
    }

    #[test]
    fn validate_client_dir_reports_missing_data_dir() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");

        let installation = validate_client_dir(install_dir).expect("缺少 data 也应返回安装记录");

        assert_eq!(installation.health, ClientHealth::MissingDataDir);
    }

    #[test]
    fn validate_client_dir_rejects_non_directory_input() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let file_path = temp_dir.path().join("DDNet.exe");
        fs::write(&file_path, b"").expect("测试文件应写入成功");

        let result = validate_client_dir(&file_path);

        assert!(result.is_err());
    }

    #[test]
    fn validate_client_dir_uses_canonical_path_for_stable_id() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let direct = validate_client_dir(install_dir).expect("直接路径应验证成功");
        let dotted = validate_client_dir(&install_dir.join(".")).expect("等价路径应验证成功");

        assert_eq!(direct.id, dotted.id);
    }

    #[test]
    fn normalize_path_replaces_backslashes_with_forward_slashes() {
        assert_eq!(
            normalize_path(Path::new(r"C:\Games\QmClient\DDNet.exe")),
            "C:/Games/QmClient/DDNet.exe"
        );
    }

    #[test]
    fn stable_installation_id_uses_fixed_fnv1a64_value() {
        assert_eq!(
            stable_installation_id("C:/Games/QmClient"),
            "qmclient-0010748f07d5a3e0"
        );
    }
}
