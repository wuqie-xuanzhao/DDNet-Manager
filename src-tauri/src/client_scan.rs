use crate::error::ManagerError;
use crate::models::{
    ClientCompatibility, ClientConfidence, ClientHealth, ClientInstallSource, ClientInstallation,
    CompatibilityReason, CompatibilityStatus,
};
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A_64_PRIME: u64 = 0x100000001b3;
const DDNET_EXECUTABLE_NAMES: &[&str] = &["DDNet.exe", "ddnet.exe", "DDNet", "ddnet"];
/// 超过此大小的 exe 不计算 sha256（避免对超大文件耗时）。
/// DDNet 客户端 exe 通常 < 100 MB，1 GB 是宽松上限。
const EXE_SHA256_MAX_SIZE: u64 = 1024 * 1024 * 1024;

/// 验证 DDNet 兼容客户端目录，并返回可供前端展示的安装记录。
///
/// 身份识别优先级：sha256 命中 catalog known_hashes > PE VS_VERSION_INFO 元信息 >
/// 路径关键字匹配 > third_party fallback。所有识别方式失败时 PE 字段仍写入
/// ClientInstallation 供前端展示。
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

    // 读 PE 元信息（仅 Windows PE 文件能解析；非 PE 静默 fallback 到 None）
    let pe_info = read_pe_version_info_safe(&executable_path);
    // 计算 sha256（仅 ≤ 1 GB，避免巨型文件耗时）
    let exe_sha256 = compute_exe_sha256_if_small(&executable_path);

    let identity = infer_client_identity(
        path,
        pe_info.as_ref().and_then(|v| v.company_name.as_deref()),
        pe_info.as_ref().and_then(|v| v.product_name.as_deref()),
        exe_sha256.as_deref(),
    );

    let health = detect_client_health(&executable_path, &storage_cfg_path, &data_dir);
    let missing_items = missing_items_for_health(&health);
    let confidence = confidence_for_identity(&identity, &health);

    Ok(ClientInstallation {
        id: stable_installation_id(&identity.client_id, &id_seed),
        client_id: identity.client_id,
        display_name: identity.display_name,
        install_dir,
        executable_path: normalize_path(&executable_path),
        storage_cfg_path: normalize_path(&storage_cfg_path),
        data_dir: normalize_path(&data_dir),
        user_data_dir: find_ddnet_user_data_dir(),
        version: identity.version,
        is_default: false,
        health,
        missing_items,
        install_source: identity.install_source,
        confidence,
        manager_owned: false,
        compatibility: detect_client_compatibility(&executable_path),
        upstream_url: identity.upstream_url,
        pe_company_name: pe_info.as_ref().and_then(|v| v.company_name.clone()),
        pe_product_name: pe_info.as_ref().and_then(|v| v.product_name.clone()),
        pe_file_version: pe_info.as_ref().and_then(|v| v.file_version.clone()),
        exe_sha256,
        last_scanned_at: Some(current_utc_rfc3339()),
    })
}

/// 判断路径是否属于本仓库本地 smoke 自动验收生成的临时客户端目录。
pub(crate) fn is_local_smoke_tmp_path(candidate: &Path) -> bool {
    let normalized = normalize_id_seed(&normalize_path(candidate));
    normalized.contains("/tmp/tauri-update-smoke/")
}

/// 简化路径用于排除路径比较：统一分隔符为 `/`，去末尾 `/`，Windows 下大小写不敏感。
///
/// 与 [`normalize_id_seed`] 的区别：本函数不调 canonicalize，纯字符串归一化，
/// 适合对可能不存在的路径做比较（排除路径列表中的路径可能已删除）。
pub(crate) fn normalize_for_compare(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        s.trim_end_matches('/').to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        s.trim_end_matches('/').to_string()
    }
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

/// 检测客户端在当前机器上的启动兼容性。
///
/// 执行以下静态检查（不实际启动进程，避免扫描时弹出游戏窗口）：
/// 1. Windows 系统版本（需 Windows 7+，即 major >= 6）。
/// 2. 可执行文件架构与当前系统是否匹配（64 位 exe 不能在 32 位 Windows 运行）。
///
/// 动态验证（`launch_verified`）不在扫描时执行，由 `process.rs` 的受控启动
/// 探测在需要时填充。
fn detect_client_compatibility(executable_path: &Path) -> ClientCompatibility {
    let mut reasons = Vec::new();
    let mut can_launch = true;

    // 1. 系统版本检查（Windows only）
    #[cfg(windows)]
    if let Some((major, minor)) = windows_version() {
        if major < 6 {
            can_launch = false;
            reasons.push(CompatibilityReason {
                code: "windows_version_too_old".to_string(),
                message: format!(
                    "Windows 版本过低（{major}.{minor}），需要 Windows 7 或更高版本。"
                ),
            });
        }
    }

    // 2. PE 架构检查
    if let Some(arch) = pe_architecture(executable_path) {
        #[cfg(windows)]
        if arch == "x86_64" && !is_64bit_windows() {
            can_launch = false;
            reasons.push(CompatibilityReason {
                code: "architecture_mismatch".to_string(),
                message: "64 位客户端无法在 32 位 Windows 上运行。".to_string(),
            });
        }
    }

    let status = if can_launch {
        CompatibilityStatus::Supported
    } else {
        CompatibilityStatus::Unsupported
    };

    ClientCompatibility {
        status,
        can_launch,
        launch_verified: false,
        reasons,
        last_launch_result: None,
    }
}

/// 读取当前 Windows 版本（major, minor）。非 Windows 平台返回 `None`。
#[cfg(windows)]
fn windows_version() -> Option<(u32, u32)> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let settings = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        .ok()?;
    let major: u32 = settings.get_value("CurrentMajorVersionNumber").ok()?;
    let minor: u32 = settings.get_value("CurrentMinorVersionNumber").ok()?;
    Some((major, minor))
}

#[cfg(not(windows))]
fn windows_version() -> Option<(u32, u32)> {
    None
}

/// 判断当前系统是否为 64 位 Windows。非 Windows 平台按编译目标指针宽度判断。
#[cfg(windows)]
fn is_64bit_windows() -> bool {
    std::env::var("PROCESSOR_ARCHITECTURE")
        .map(|v| v.eq_ignore_ascii_case("AMD64") || v.eq_ignore_ascii_case("IA64"))
        .unwrap_or(false)
        || std::env::var("PROCESSOR_ARCHITEW6432")
            .map(|v| v.eq_ignore_ascii_case("AMD64"))
            .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_64bit_windows() -> bool {
    cfg!(target_pointer_width = "64")
}

/// 解析 PE 文件的 COFF Machine 字段，返回架构标识。
///
/// 读取 PE header：
/// - `0x014c` = x86 (Intel 386)
/// - `0x8664` = x86_64 (AMD64)
///
/// 非 PE 文件或解析失败返回 `None`。
fn pe_architecture(path: &Path) -> Option<&'static str> {
    use std::io::{Read, Seek, SeekFrom};

    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);

    // DOS header magic
    let mut dos_magic = [0u8; 2];
    reader.read_exact(&mut dos_magic).ok()?;
    if dos_magic != [b'M', b'Z'] {
        return None;
    }

    // PE header offset at 0x3C
    reader.seek(SeekFrom::Start(0x3C)).ok()?;
    let mut pe_offset_buf = [0u8; 4];
    reader.read_exact(&mut pe_offset_buf).ok()?;
    let pe_offset = u32::from_le_bytes(pe_offset_buf) as u64;

    // PE signature
    reader.seek(SeekFrom::Start(pe_offset)).ok()?;
    let mut pe_sig = [0u8; 4];
    reader.read_exact(&mut pe_sig).ok()?;
    if &pe_sig != b"PE\0\0" {
        return None;
    }

    // COFF header machine field (2 bytes after PE signature)
    let mut machine_buf = [0u8; 2];
    reader.read_exact(&mut machine_buf).ok()?;
    let machine = u16::from_le_bytes(machine_buf);

    match machine {
        0x014c => Some("x86"),
        0x8664 => Some("x86_64"),
        _ => None,
    }
}

fn missing_items_for_health(health: &ClientHealth) -> Vec<String> {
    match health {
        ClientHealth::Ok => Vec::new(),
        ClientHealth::MissingExecutable => vec!["executable".to_string()],
        ClientHealth::MissingStorageCfg => vec!["storage.cfg".to_string()],
        ClientHealth::MissingDataDir => vec!["data".to_string()],
    }
}

fn confidence_for_identity(identity: &ClientIdentity, health: &ClientHealth) -> ClientConfidence {
    if health != &ClientHealth::Ok {
        return ClientConfidence::Partial;
    }
    match identity.identity_source {
        IdentitySource::HashMatch => ClientConfidence::Verified,
        IdentitySource::PeMatch(crate::client_catalog::PeMatchStrength::Strong) => {
            ClientConfidence::Verified
        }
        // PE Weak（只匹配 CompanyName 或 ProductName 之一）：降级到 Compatible，
        // 因为单字段可能命中多个客户端（DDNet fork 通常都保留 "DDNet" ProductName）
        IdentitySource::PeMatch(crate::client_catalog::PeMatchStrength::Weak) => {
            ClientConfidence::Compatible
        }
        IdentitySource::PathMatch => ClientConfidence::Verified,
        IdentitySource::Unknown => ClientConfidence::Compatible,
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
    /// 来自 sha256 命中或 PE file_version 的版本号。
    version: Option<String>,
    /// 识别来源（驱动 confidence 计算）。
    identity_source: IdentitySource,
}

/// 客户端身份识别的来源。决定 ClientConfidence 计算。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdentitySource {
    /// sha256 命中 catalog known_hashes（最高置信度，不可伪造）。
    HashMatch,
    /// PE VS_VERSION_INFO 元信息匹配。
    PeMatch(crate::client_catalog::PeMatchStrength),
    /// 路径关键字匹配（catalog.aliases）。
    PathMatch,
    /// 所有匹配方式都未命中，third_party fallback。
    Unknown,
}

fn infer_client_identity(
    path: &Path,
    pe_company: Option<&str>,
    pe_product: Option<&str>,
    exe_sha256: Option<&str>,
) -> ClientIdentity {
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

    // 1. Steam DDNet 强制识别（路径权威，跳过其他匹配）
    if is_steam_ddnet_path(&haystack) {
        return ClientIdentity {
            client_id: "ddnet".to_string(),
            display_name: "DDNet".to_string(),
            install_source,
            upstream_url: Some(crate::client_catalog::ddnet_steam_url().to_string()),
            version: None,
            identity_source: IdentitySource::PathMatch,
        };
    }

    // 2. sha256 命中 catalog known_hashes（最高优先级，不可伪造）
    if let Some(hash) = exe_sha256 {
        if let Some((entry, version)) = crate::client_catalog::match_catalog_by_hash(hash) {
            return ClientIdentity {
                client_id: entry.client_id.to_string(),
                display_name: entry.display_name.to_string(),
                install_source,
                upstream_url: entry.upstream_url.map(str::to_string),
                version: Some(version.to_string()),
                identity_source: IdentitySource::HashMatch,
            };
        }
    }

    // 3. PE VS_VERSION_INFO 元信息匹配（优于路径匹配）
    if let Some((entry, strength)) =
        crate::client_catalog::match_catalog_by_pe(pe_company, pe_product)
    {
        return ClientIdentity {
            client_id: entry.client_id.to_string(),
            display_name: entry.display_name.to_string(),
            install_source,
            upstream_url: entry.upstream_url.map(str::to_string),
            version: None,
            identity_source: IdentitySource::PeMatch(strength),
        };
    }

    // 4. 路径关键字匹配（fallback）
    if let Some(entry) = crate::client_catalog::match_catalog_entry(&haystack) {
        return ClientIdentity {
            client_id: entry.client_id.to_string(),
            display_name: entry.display_name.to_string(),
            install_source,
            upstream_url: entry.upstream_url.map(str::to_string),
            version: None,
            identity_source: IdentitySource::PathMatch,
        };
    }

    // 5. third_party fallback
    ClientIdentity {
        client_id: "third_party".to_string(),
        display_name: trim_app_extension(name).to_string(),
        install_source,
        upstream_url: None,
        version: pe_company.map(|_| "unknown".to_string()).or(None),
        identity_source: IdentitySource::Unknown,
    }
}

/// 读 PE VS_VERSION_INFO；任何错误（非 PE、资源缺失、解析失败）静默返回 None。
/// 这样不影响扫描主流程，PE 元信息只是辅助识别手段。
fn read_pe_version_info_safe(exe_path: &Path) -> Option<ntfs_search::VersionInfo> {
    ntfs_search::read_version_info(exe_path).ok()
}

/// 计算 exe 文件的 SHA-256（小写十六进制）。仅当文件 ≤ EXE_SHA256_MAX_SIZE 时计算，
/// 避免对超大文件耗时。任何 I/O 错误静默返回 None。
/// 复用 [`crate::download::verify::compute_file_sha256_hex`] 的缓冲区分配逻辑（review issue #14）。
fn compute_exe_sha256_if_small(exe_path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(exe_path).ok()?;
    if metadata.len() > EXE_SHA256_MAX_SIZE {
        return None;
    }
    crate::download::verify::compute_file_sha256_hex(exe_path).ok()
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
