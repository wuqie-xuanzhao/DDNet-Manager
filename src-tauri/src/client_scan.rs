use crate::error::ManagerError;
use crate::models::{
    ClientCompatibility, ClientConfidence, ClientHealth, ClientInstallSource, ClientInstallation,
};
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A_64_PRIME: u64 = 0x100000001b3;
const DDNET_EXECUTABLE_NAMES: &[&str] = &["DDNet.exe", "ddnet.exe", "DDNet", "ddnet"];

/// 验证 DDNet 兼容客户端目录，并返回可供前端展示的安装记录。
pub fn validate_client_dir(path: &Path) -> Result<ClientInstallation, ManagerError> {
    if !path.is_dir() {
        return Err(ManagerError::NotFound(format!(
            "client path is not a directory: {}",
            normalize_path(path)
        )));
    }

    let executable_path =
        find_ddnet_executable(path).unwrap_or_else(|| default_executable_path(path));
    let storage_cfg_path = find_storage_cfg_path(path);
    let data_dir = find_data_dir(path);
    let install_dir = normalize_path(path);
    let id_seed = normalized_id_seed(path);
    let identity = infer_client_identity(path);

    let health = detect_client_health(&executable_path, &storage_cfg_path, &data_dir);
    let missing_items = missing_items_for_health(&health);
    let confidence = confidence_for_health(&identity.client_id, &health);
    let can_launch = health == ClientHealth::Ok;

    Ok(ClientInstallation {
        id: stable_installation_id(&identity.client_id, &id_seed),
        client_id: identity.client_id,
        display_name: identity.display_name,
        install_dir,
        executable_path: normalize_path(&executable_path),
        storage_cfg_path: normalize_path(&storage_cfg_path),
        data_dir: normalize_path(&data_dir),
        user_data_dir: find_ddnet_user_data_dir(),
        version: None,
        is_default: false,
        health,
        missing_items,
        install_source: identity.install_source,
        confidence,
        manager_owned: false,
        compatibility: ClientCompatibility {
            can_launch,
            ..ClientCompatibility::default()
        },
        upstream_url: identity.upstream_url,
        last_scanned_at: Some(current_utc_rfc3339()),
    })
}

/// 判断路径是否属于本仓库本地 smoke 自动验收生成的临时客户端目录。
pub(crate) fn is_local_smoke_tmp_path(candidate: &Path) -> bool {
    let normalized = normalize_id_seed(&normalize_path(candidate));
    normalized.contains("/tmp/tauri-update-smoke/")
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

fn missing_items_for_health(health: &ClientHealth) -> Vec<String> {
    match health {
        ClientHealth::Ok => Vec::new(),
        ClientHealth::MissingExecutable => vec!["executable".to_string()],
        ClientHealth::MissingStorageCfg => vec!["storage.cfg".to_string()],
        ClientHealth::MissingDataDir => vec!["data".to_string()],
    }
}

fn confidence_for_health(client_id: &str, health: &ClientHealth) -> ClientConfidence {
    match health {
        ClientHealth::Ok if client_id == "third_party" => ClientConfidence::Compatible,
        ClientHealth::Ok => ClientConfidence::Verified,
        _ => ClientConfidence::Partial,
    }
}

fn find_ddnet_executable(path: &Path) -> Option<PathBuf> {
    if is_macos_app_bundle(path) {
        return find_macos_app_executable(path);
    }

    let entries = std::fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if DDNET_EXECUTABLE_NAMES
            .iter()
            .any(|expected| file_name.eq_ignore_ascii_case(expected))
            && entry.path().is_file()
        {
            return Some(entry.path());
        }
    }

    None
}

fn default_executable_path(path: &Path) -> PathBuf {
    if is_macos_app_bundle(path) {
        path.join("Contents").join("MacOS").join("DDNet")
    } else if cfg!(target_os = "windows") {
        path.join("DDNet.exe")
    } else {
        path.join("DDNet")
    }
}

fn find_macos_app_executable(path: &Path) -> Option<PathBuf> {
    let macos_dir = path.join("Contents").join("MacOS");
    let entries = std::fs::read_dir(macos_dir).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn find_storage_cfg_path(path: &Path) -> PathBuf {
    let bundle_resource_cfg = path.join("Contents").join("Resources").join("storage.cfg");
    if is_macos_app_bundle(path) && bundle_resource_cfg.is_file() {
        bundle_resource_cfg
    } else {
        path.join("storage.cfg")
    }
}

fn find_data_dir(path: &Path) -> PathBuf {
    let bundle_resource_data = path.join("Contents").join("Resources").join("data");
    if is_macos_app_bundle(path) && bundle_resource_data.is_dir() {
        bundle_resource_data
    } else {
        path.join("data")
    }
}

fn is_macos_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
}

struct ClientIdentity {
    client_id: String,
    display_name: String,
    install_source: ClientInstallSource,
    upstream_url: Option<String>,
}

fn infer_client_identity(path: &Path) -> ClientIdentity {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("DDNet Client");
    let haystack = normalize_path(path).to_ascii_lowercase();
    let install_source = if is_steam_ddnet_path(&haystack) {
        ClientInstallSource::Steam
    } else {
        ClientInstallSource::Manual
    };

    if is_steam_ddnet_path(&haystack) {
        return ClientIdentity {
            client_id: "ddnet".to_string(),
            display_name: "DDNet".to_string(),
            install_source,
            upstream_url: Some(crate::client_catalog::ddnet_steam_url().to_string()),
        };
    }

    if let Some(entry) = crate::client_catalog::match_catalog_entry(&haystack) {
        return ClientIdentity {
            client_id: entry.client_id.to_string(),
            display_name: entry.display_name.to_string(),
            install_source,
            upstream_url: entry.upstream_url.map(str::to_string),
        };
    }

    ClientIdentity {
        client_id: "third_party".to_string(),
        display_name: trim_app_extension(name).to_string(),
        install_source,
        upstream_url: None,
    }
}

fn is_steam_ddnet_path(normalized_lower_path: &str) -> bool {
    normalized_lower_path.contains("/steamapps/common/ddnet")
}

fn trim_app_extension(name: &str) -> &str {
    name.strip_suffix(".app").unwrap_or(name)
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

fn stable_installation_id(client_id: &str, path: &str) -> String {
    let hash = path
        .as_bytes()
        .iter()
        .fold(FNV1A_64_OFFSET_BASIS, |hash, byte| {
            let mixed = hash ^ u64::from(*byte);
            mixed.wrapping_mul(FNV1A_64_PRIME)
        });

    format!("{client_id}-{hash:016x}")
}

fn current_utc_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
#[path = "test/client_scan.rs"]
mod tests;
