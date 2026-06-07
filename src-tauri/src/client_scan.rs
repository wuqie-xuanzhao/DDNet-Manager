use crate::models::{ClientHealth, ClientInstallation};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A_64_PRIME: u64 = 0x100000001b3;
const DDNET_EXECUTABLE_NAMES: &[&str] = &["DDNet.exe", "ddnet.exe"];

#[derive(Debug, Error)]
enum ClientScanError {
    #[error("客户端路径不是目录: {0}")]
    NotDirectory(String),
}

/// 表示客户端扫描使用的后端内部选项。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanOptions {
    /// 扫描根目录列表。
    pub roots: Vec<PathBuf>,
    /// 是否包含已保存路径。该字段由 command 层补充具体路径。
    pub include_saved_paths: bool,
    /// 是否进行更深层级扫描。
    pub deep: bool,
    /// 是否启用 Everything provider 作为扫描加速。
    pub use_everything: bool,
}

struct CommonScanRootEnv<'a> {
    user_profile: Option<&'a Path>,
    program_files: Option<&'a Path>,
    program_files_x86: Option<&'a Path>,
    local_appdata: Option<&'a Path>,
    appdata: Option<&'a Path>,
    program_data: Option<&'a Path>,
}

/// 验证 DDNet 兼容客户端目录，并返回可供前端展示的安装记录。
pub fn validate_client_dir(path: &Path) -> Result<ClientInstallation, String> {
    if !path.is_dir() {
        return Err(ClientScanError::NotDirectory(normalize_path(path)).to_string());
    }

    let executable_path = find_ddnet_executable(path).unwrap_or_else(|| path.join("DDNet.exe"));
    let storage_cfg_path = path.join("storage.cfg");
    let data_dir = path.join("data");
    let install_dir = normalize_path(path);
    let id_seed = normalized_id_seed(path);
    let (client_id, display_name) = infer_client_identity(path);

    let health = detect_client_health(&executable_path, &storage_cfg_path, &data_dir);

    Ok(ClientInstallation {
        id: stable_installation_id(&client_id, &id_seed),
        client_id,
        display_name,
        install_dir,
        executable_path: normalize_path(&executable_path),
        storage_cfg_path: normalize_path(&storage_cfg_path),
        data_dir: normalize_path(&data_dir),
        user_data_dir: find_ddnet_user_data_dir(),
        version: None,
        is_default: false,
        health,
        last_scanned_at: Some(current_utc_rfc3339()),
    })
}

/// 在指定根目录下扫描 DDNet 兼容客户端候选安装目录。
pub fn scan_client_installations(options: &ScanOptions) -> Result<Vec<ClientInstallation>, String> {
    let mut installations = Vec::new();
    let mut seen_ids = HashSet::new();
    let max_depth = if options.deep { 5 } else { 3 };

    for root in scan_roots(options) {
        if !root.is_dir() {
            continue;
        }

        for candidate in find_candidate_dirs(&root, max_depth)? {
            let installation = validate_client_dir(&candidate)?;
            if seen_ids.insert(installation.id.clone()) {
                installations.push(installation);
            }
        }
    }

    if options.use_everything {
        for candidate in find_everything_candidate_dirs() {
            if !candidate.is_dir() {
                continue;
            }
            let installation = validate_client_dir(&candidate)?;
            if seen_ids.insert(installation.id.clone()) {
                installations.push(installation);
            }
        }
    }

    installations.sort_by(|left, right| left.install_dir.cmp(&right.install_dir));
    Ok(installations)
}

/// 返回 Windows 优先的轻量默认扫描根。
pub fn default_scan_roots() -> Vec<PathBuf> {
    let user_profile = std::env::var_os("USERPROFILE");
    let program_files = std::env::var_os("ProgramFiles");
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)");
    let local_appdata = std::env::var_os("LOCALAPPDATA");
    let appdata = std::env::var_os("APPDATA");
    let program_data = std::env::var_os("ProgramData");

    let mut roots = common_scan_roots_from_env_values(CommonScanRootEnv {
        user_profile: user_profile.as_deref().map(Path::new),
        program_files: program_files.as_deref().map(Path::new),
        program_files_x86: program_files_x86.as_deref().map(Path::new),
        local_appdata: local_appdata.as_deref().map(Path::new),
        appdata: appdata.as_deref().map(Path::new),
        program_data: program_data.as_deref().map(Path::new),
    });
    roots.extend(find_steam_library_ddnet_roots());

    if let Some(data_dir) = dirs::data_dir() {
        push_unique_root(&mut roots, data_dir.join("DDNet"));
        push_unique_root(&mut roots, data_dir.join("QmClient"));
    }

    roots
}

fn common_scan_roots_from_env_values(env: CommonScanRootEnv<'_>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(user_profile) = env.user_profile {
        for child in ["Downloads", "Desktop", "Documents", "Games"] {
            push_unique_root(&mut roots, user_profile.join(child));
        }
    }

    for root in [
        env.program_files.map(Path::to_path_buf),
        env.program_files_x86.map(Path::to_path_buf),
        env.local_appdata.map(Path::to_path_buf),
        env.appdata.map(Path::to_path_buf),
        Some(PathBuf::from("C:/Games")),
        Some(PathBuf::from("D:/Games")),
        Some(PathBuf::from(
            "C:/Program Files (x86)/Steam/steamapps/common/DDNet",
        )),
        Some(PathBuf::from(
            "C:/Program Files/Steam/steamapps/common/DDNet",
        )),
        Some(PathBuf::from("D:/SteamLibrary/steamapps/common/DDNet")),
        Some(PathBuf::from("E:/SteamLibrary/steamapps/common/DDNet")),
        Some(PathBuf::from("F:/SteamLibrary/steamapps/common/DDNet")),
    ]
    .into_iter()
    .flatten()
    {
        push_unique_root(&mut roots, root);
    }

    if let Some(program_data) = env.program_data {
        push_unique_root(
            &mut roots,
            program_data
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    roots
}

fn push_unique_root(roots: &mut Vec<PathBuf>, root: PathBuf) {
    if !roots.iter().any(|existing| existing == &root) {
        roots.push(root);
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

fn scan_roots(options: &ScanOptions) -> Vec<PathBuf> {
    if options.roots.is_empty() {
        default_scan_roots()
    } else {
        options.roots.clone()
    }
}

fn find_candidate_dirs(root: &Path, max_depth: usize) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    let mut queue = VecDeque::from([(root.to_path_buf(), 0_usize)]);

    while let Some((dir, depth)) = queue.pop_front() {
        if is_candidate_dir(&dir) {
            candidates.push(dir);
            continue;
        }

        if depth >= max_depth {
            continue;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                if depth == 0 {
                    return Err(format!("failed to scan {}: {error}", dir.display()));
                }
                continue;
            }
        };

        for entry in entries {
            let entry = entry.map_err(|error| format!("failed to read scan entry: {error}"))?;
            let path = entry.path();
            if path.is_dir() {
                queue.push_back((path, depth + 1));
            }
        }
    }

    Ok(candidates)
}

fn is_candidate_dir(path: &Path) -> bool {
    find_ddnet_executable(path).is_some() || path.join("storage.cfg").is_file()
}

fn find_ddnet_executable(path: &Path) -> Option<PathBuf> {
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

fn find_everything_candidate_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for es_path in everything_executable_candidates() {
        for query in [
            "DDNet.exe",
            "ddnet.exe",
            "storage.cfg",
            "settings_ddnet.cfg",
        ] {
            let Some(text) = run_everything_query(&es_path, query) else {
                continue;
            };
            for candidate in everything_candidate_dirs_from_output(&text) {
                if seen.insert(normalize_path(&candidate)) {
                    candidates.push(candidate);
                }
            }
        }
    }

    candidates
}

fn everything_executable_candidates() -> Vec<PathBuf> {
    [
        PathBuf::from("C:/Program Files/Everything/es.exe"),
        PathBuf::from("C:/Program Files (x86)/Everything/es.exe"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect()
}

fn run_everything_query(es_path: &Path, query: &str) -> Option<String> {
    let mut child = Command::new(es_path)
        .args(["-n", "200", query])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started_at = Instant::now();

    loop {
        if child.try_wait().ok()?.is_some() {
            let output = child.wait_with_output().ok()?;
            if !output.status.success() {
                return None;
            }
            return Some(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        if started_at.elapsed() >= Duration::from_secs(2) {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn everything_candidate_dirs_from_output(output: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let path = PathBuf::from(line);
        let Some(parent) = path.parent() else {
            continue;
        };
        let candidate = parent.to_path_buf();
        if seen.insert(normalize_path(&candidate)) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn find_steam_library_ddnet_roots() -> Vec<PathBuf> {
    steam_libraryfolders_paths()
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .flat_map(|content| steam_ddnet_roots_from_libraryfolders_text(&content))
        .collect()
}

fn steam_libraryfolders_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("C:/Program Files (x86)/Steam/steamapps/libraryfolders.vdf"),
        PathBuf::from("C:/Program Files/Steam/steamapps/libraryfolders.vdf"),
    ];

    if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
        paths.push(
            PathBuf::from(program_files)
                .join("Steam")
                .join("steamapps")
                .join("libraryfolders.vdf"),
        );
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        paths.push(
            PathBuf::from(program_files)
                .join("Steam")
                .join("steamapps")
                .join("libraryfolders.vdf"),
        );
    }

    paths
}

fn steam_ddnet_roots_from_libraryfolders_text(content: &str) -> Vec<PathBuf> {
    content
        .lines()
        .filter_map(parse_steam_library_path_line)
        .map(|path| path.join("steamapps").join("common").join("DDNet"))
        .collect()
}

fn parse_steam_library_path_line(line: &str) -> Option<PathBuf> {
    let mut parts = line.split('"').filter(|part| !part.trim().is_empty());
    let key = parts.next()?.trim();
    let value = parts.next()?.trim();
    if key == "path" {
        Some(PathBuf::from(value.replace("\\\\", "\\")))
    } else {
        None
    }
}

fn infer_client_identity(path: &Path) -> (String, String) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("DDNet Client");
    let haystack = normalize_path(path).to_ascii_lowercase();

    if haystack.contains("qmclient") && haystack.contains("nightly") {
        return (
            "qmclient_nightly".to_string(),
            "QmClient Nightly".to_string(),
        );
    }
    if haystack.contains("qmclient") || haystack.contains("qm-client") {
        return ("qmclient".to_string(), "QmClient".to_string());
    }
    if haystack.contains("taterclient") || haystack.contains("tclient") {
        return ("taterclient".to_string(), "TaterClient".to_string());
    }
    if haystack.contains("bestclient") || haystack.contains("best-client") {
        return ("bestclient".to_string(), "BestClient".to_string());
    }
    if haystack.contains("cactus") {
        return ("cactusclient".to_string(), "Cactus Client".to_string());
    }
    if haystack.contains("steamapps/common/ddnet") || haystack.ends_with("/ddnet") {
        return ("ddnet_vanilla".to_string(), "DDNet".to_string());
    }

    ("third_party".to_string(), name.to_string())
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
mod tests;
