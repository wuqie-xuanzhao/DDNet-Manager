use crate::models::{ClientUpdateCheck, DownloadJob, DownloadJobStatus};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

const MAX_ZIP_FILES: usize = 20_000;
const MAX_UNPACKED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_DOWNLOAD_REDIRECTS: usize = 5;
const TRUSTED_DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "raw.githubusercontent.com",
    "ddnet.org",
];

/// 表示更新资产的安装包类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// zip 压缩包，当前支持自动解压安装。
    Zip,
    /// Linux tar.xz 压缩包，当前仅允许手动安装。
    TarXz,
    /// macOS dmg 镜像包，当前仅允许手动安装。
    Dmg,
    /// 未识别后缀，保守禁止自动安装。
    Unknown,
}

/// 表示一次下载文件写入请求。
pub struct DownloadFileRequest<'a> {
    /// 资产下载地址。
    pub asset_url: &'a str,
    /// 缓存文件路径。
    pub cache_path: &'a Path,
    /// manifest 中声明的期望大小。
    pub expected_size: u64,
    /// 用户显式启用的代理或镜像下载 host。
    pub enabled_route_hosts: &'a [String],
}

/// 管理当前进程内的下载任务状态。
#[derive(Clone, Default)]
pub struct DownloadManager {
    jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
}

impl DownloadManager {
    /// 插入新的下载任务。
    pub fn insert(&self, job: DownloadJob) -> Result<(), String> {
        self.jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?
            .insert(job.id.clone(), job);
        Ok(())
    }

    /// 读取指定下载任务。
    pub fn get(&self, job_id: &str) -> Result<Option<DownloadJob>, String> {
        Ok(self
            .jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?
            .get(job_id)
            .cloned())
    }

    /// 更新指定下载任务。
    pub fn update<F>(&self, job_id: &str, update: F) -> Result<DownloadJob, String>
    where
        F: FnOnce(&mut DownloadJob),
    {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| format!("download job not found: {job_id}"))?;
        update(job);
        Ok(job.clone())
    }

    /// 标记下载任务已取消。
    pub fn cancel(&self, job_id: &str) -> Result<DownloadJob, String> {
        self.update(job_id, |job| {
            job.status = DownloadJobStatus::Canceled;
        })
    }
}

/// 计算输入字节的 SHA-256 小写十六进制摘要。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

/// 基于更新请求和下载目录创建下载任务模型。
pub fn create_download_job(
    client_installation_id: &str,
    update: &ClientUpdateCheck,
    downloads_dir: &Path,
) -> DownloadJob {
    let now = OffsetDateTime::now_utc().unix_timestamp_nanos();
    let id = format!("download-{now}");
    let cache_path = downloads_dir.join(format!(
        "{id}{}",
        package_kind_for_asset_url(&update.asset.asset_url).cache_suffix()
    ));
    DownloadJob {
        id,
        client_installation_id: client_installation_id.to_string(),
        client_id: update.client_id.clone(),
        channel: update.channel.clone(),
        version: update.latest_version.clone(),
        asset_url: update.asset.asset_url.clone(),
        sha256: update.asset.sha256.clone(),
        size: update.asset.size,
        status: DownloadJobStatus::Pending,
        downloaded_bytes: 0,
        cache_path: normalize_path(&cache_path),
        error: None,
    }
}

/// 根据下载资产 URL 推断安装包类型。
pub fn package_kind_for_asset_url(asset_url: &str) -> PackageKind {
    let asset_url = asset_url.to_ascii_lowercase();
    if asset_url.ends_with(".tar.xz") {
        PackageKind::TarXz
    } else if asset_url.ends_with(".dmg") {
        PackageKind::Dmg
    } else if asset_url.ends_with(".zip") {
        PackageKind::Zip
    } else {
        PackageKind::Unknown
    }
}

/// 校验当前自动安装链路是否支持该安装包类型。
pub fn auto_install_guard(package_kind: PackageKind) -> Result<(), String> {
    match package_kind {
        PackageKind::Zip | PackageKind::TarXz | PackageKind::Dmg => Ok(()),
        PackageKind::Unknown => Err(
            "automatic install only supports .zip, .tar.xz, and .dmg packages; unknown package type requires manual install"
                .to_string(),
        ),
    }
}

/// 下载远程资产到缓存文件，并通过回调报告已下载字节数。
pub async fn download_asset_to_file<F>(
    request: DownloadFileRequest<'_>,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64) -> bool + Send,
{
    validate_download_url_with_hosts(request.asset_url, request.enabled_route_hosts)?;
    let part_path = part_file_path(request.cache_path);
    if let Some(parent) = request.cache_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create download cache dir: {error}"))?;
    }
    if part_path.exists() {
        fs::remove_file(&part_path)
            .map_err(|error| format!("failed to clear partial download file: {error}"))?;
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("failed to create download client: {error}"))?;
    let mut response =
        send_download_request(&client, request.asset_url, request.enabled_route_hosts).await?;

    if response
        .content_length()
        .is_some_and(|length| length != request.expected_size)
    {
        return Err("download content length does not match manifest size".to_string());
    }

    let mut file = fs::File::create(&part_path)
        .map_err(|error| format!("failed to create download cache file: {error}"))?;
    let mut downloaded = 0_u64;

    loop {
        let next_chunk = match response.chunk().await {
            Ok(chunk) => chunk,
            Err(error) => {
                let _ = fs::remove_file(&part_path);
                return Err(format!("failed to read download stream: {error}"));
            }
        };
        let Some(chunk) = next_chunk else {
            break;
        };
        downloaded = match downloaded.checked_add(chunk.len() as u64) {
            Some(value) => value,
            None => {
                let _ = fs::remove_file(&part_path);
                return Err("downloaded byte count overflow".to_string());
            }
        };
        if downloaded > request.expected_size {
            let _ = fs::remove_file(&part_path);
            return Err("downloaded bytes exceed manifest size".to_string());
        }
        if let Err(error) = file.write_all(&chunk) {
            let _ = fs::remove_file(&part_path);
            return Err(format!("failed to write download cache file: {error}"));
        }
        if !on_progress(downloaded) {
            let _ = fs::remove_file(&part_path);
            return Err("download canceled".to_string());
        }
    }

    drop(file);
    if request.cache_path.exists() {
        fs::remove_file(request.cache_path)
            .map_err(|error| format!("failed to replace existing cache file: {error}"))?;
    }
    fs::rename(&part_path, request.cache_path)
        .map_err(|error| format!("failed to finalize download cache file: {error}"))
}

/// 校验已下载文件的字节数和 SHA-256 摘要。
pub fn verify_downloaded_file(
    path: &Path,
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read download file: {error}"))?;
    let actual_size = bytes.len() as u64;
    if actual_size != expected_size {
        return Err(format!(
            "download size mismatch: expected {expected_size}, got {actual_size}"
        ));
    }

    let actual_sha256 = sha256_hex(&bytes);
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(format!(
            "download sha256 mismatch: expected {expected_sha256}, got {actual_sha256}"
        ));
    }

    Ok(())
}

/// 将 zip 包安全解压到 staging 目录，拒绝路径穿越和绝对路径。
pub fn extract_zip_to_staging(zip_path: &Path, staging_dir: &Path) -> Result<(), String> {
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir)
            .map_err(|error| format!("failed to clear staging dir: {error}"))?;
    }
    fs::create_dir_all(staging_dir)
        .map_err(|error| format!("failed to create staging dir: {error}"))?;

    let zip_file =
        fs::File::open(zip_path).map_err(|error| format!("failed to open zip file: {error}"))?;
    let mut archive =
        zip::ZipArchive::new(zip_file).map_err(|error| format!("invalid zip file: {error}"))?;

    if archive.len() > MAX_ZIP_FILES {
        return Err(format!("zip contains more than {MAX_ZIP_FILES} files"));
    }

    let staging_root = fs::canonicalize(staging_dir)
        .map_err(|error| format!("failed to canonicalize staging dir: {error}"))?;
    let mut unpacked_bytes = 0_u64;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("failed to read zip entry: {error}"))?;
        let enclosed_name = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe zip entry path: {}", entry.name()))?;
        let output_path = staging_root.join(enclosed_name);
        ensure_inside_root(&staging_root, &output_path)?;

        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("failed to create zip directory: {error}"))?;
            continue;
        }

        unpacked_bytes = unpacked_bytes
            .checked_add(entry.size())
            .ok_or_else(|| "zip unpacked size overflow".to_string())?;
        if unpacked_bytes > MAX_UNPACKED_BYTES {
            return Err(format!(
                "zip unpacked size exceeds {MAX_UNPACKED_BYTES} bytes"
            ));
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create zip parent directory: {error}"))?;
        }

        let mut output = fs::File::create(&output_path)
            .map_err(|error| format!("failed to create extracted file: {error}"))?;
        copy_zip_entry(&mut entry, &mut output)?;
    }

    Ok(())
}

/// 按安装包类型安全解包或复制到 staging 目录。
pub fn extract_package_to_staging(
    package_path: &Path,
    staging_dir: &Path,
    package_kind: PackageKind,
) -> Result<(), String> {
    match package_kind {
        PackageKind::Zip => extract_zip_to_staging(package_path, staging_dir),
        PackageKind::TarXz => extract_tar_xz_to_staging(package_path, staging_dir),
        PackageKind::Dmg => extract_dmg_to_staging(package_path, staging_dir),
        PackageKind::Unknown => Err(
            "automatic install only supports .zip, .tar.xz, and .dmg packages; unknown package type requires manual install"
                .to_string(),
        ),
    }
}

/// 将 tar.xz 包安全解包到 staging 目录，拒绝绝对路径和路径穿越。
pub fn extract_tar_xz_to_staging(tar_xz_path: &Path, staging_dir: &Path) -> Result<(), String> {
    let staging_root = prepare_staging_dir(staging_dir)?;
    let file = fs::File::open(tar_xz_path)
        .map_err(|error| format!("failed to open tar.xz file: {error}"))?;
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut unpacked_files = 0_usize;
    let mut unpacked_bytes = 0_u64;

    let entries = archive
        .entries()
        .map_err(|error| format!("invalid tar.xz file: {error}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| format!("failed to read tar entry: {error}"))?;
        unpacked_files = unpacked_files
            .checked_add(1)
            .ok_or_else(|| "tar entry count overflow".to_string())?;
        if unpacked_files > MAX_ZIP_FILES {
            return Err(format!("tar contains more than {MAX_ZIP_FILES} files"));
        }
        let Some(output_path) = prepare_tar_entry(&mut entry, &staging_root)? else {
            continue;
        };
        unpacked_bytes = add_tar_entry_size(unpacked_bytes, &entry)?;
        if unpacked_bytes > MAX_UNPACKED_BYTES {
            return Err(format!(
                "tar unpacked size exceeds {MAX_UNPACKED_BYTES} bytes"
            ));
        }
        extract_tar_file_entry(&mut entry, &output_path)?;
    }

    Ok(())
}

fn extract_dmg_to_staging(dmg_path: &Path, staging_dir: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        extract_dmg_to_staging_macos(dmg_path, staging_dir)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (dmg_path, staging_dir);
        Err("automatic .dmg install requires macOS hdiutil and app bundle copy support".to_string())
    }
}

fn prepare_staging_dir(staging_dir: &Path) -> Result<PathBuf, String> {
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir)
            .map_err(|error| format!("failed to clear staging dir: {error}"))?;
    }
    fs::create_dir_all(staging_dir)
        .map_err(|error| format!("failed to create staging dir: {error}"))?;
    fs::canonicalize(staging_dir)
        .map_err(|error| format!("failed to canonicalize staging dir: {error}"))
}

fn prepare_tar_entry<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    staging_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let entry_path = entry
        .path()
        .map_err(|error| format!("failed to read tar entry path: {error}"))?
        .into_owned();
    if entry_path.is_absolute() {
        return Err(format!("unsafe tar entry path: {}", entry_path.display()));
    }
    let output_path = staging_root.join(&entry_path);
    ensure_inside_root(staging_root, &output_path)
        .map_err(|_| format!("unsafe tar entry path: {}", entry_path.display()))?;

    let entry_type = entry.header().entry_type();
    if entry_type.is_symlink() || entry_type.is_hard_link() {
        return Err(format!("unsafe tar entry path: {}", entry_path.display()));
    }
    if entry_type.is_dir() {
        fs::create_dir_all(&output_path)
            .map_err(|error| format!("failed to create tar directory: {error}"))?;
        return Ok(None);
    }
    if !entry_type.is_file() {
        return Ok(None);
    }

    Ok(Some(output_path))
}

fn add_tar_entry_size<R: Read>(
    current_size: u64,
    entry: &tar::Entry<'_, R>,
) -> Result<u64, String> {
    current_size
        .checked_add(entry.header().size().unwrap_or(0))
        .ok_or_else(|| "tar unpacked size overflow".to_string())
}

fn extract_tar_file_entry<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    output_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create tar parent directory: {error}"))?;
    }
    let mut output = fs::File::create(output_path)
        .map_err(|error| format!("failed to create extracted file: {error}"))?;
    std::io::copy(entry, &mut output)
        .map_err(|error| format!("failed to extract tar entry: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = entry.header().mode().unwrap_or(0o644) & 0o777;
        fs::set_permissions(output_path, fs::Permissions::from_mode(mode))
            .map_err(|error| format!("failed to set extracted file permissions: {error}"))?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn extract_dmg_to_staging_macos(dmg_path: &Path, staging_dir: &Path) -> Result<(), String> {
    use std::process::Command;

    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir)
            .map_err(|error| format!("failed to clear staging dir: {error}"))?;
    }
    fs::create_dir_all(staging_dir)
        .map_err(|error| format!("failed to create staging dir: {error}"))?;

    let mount_dir = staging_dir.with_extension("dmg-mount");
    if mount_dir.exists() {
        fs::remove_dir_all(&mount_dir)
            .map_err(|error| format!("failed to clear dmg mount dir: {error}"))?;
    }
    fs::create_dir_all(&mount_dir)
        .map_err(|error| format!("failed to create dmg mount dir: {error}"))?;

    let attach_status = Command::new("hdiutil")
        .arg("attach")
        .arg(dmg_path)
        .arg("-mountpoint")
        .arg(&mount_dir)
        .arg("-nobrowse")
        .arg("-readonly")
        .status()
        .map_err(|error| format!("failed to attach dmg: {error}"))?;
    if !attach_status.success() {
        let _ = fs::remove_dir_all(&mount_dir);
        return Err(format!("failed to attach dmg: {attach_status}"));
    }

    let copy_result = find_first_app_bundle(&mount_dir).and_then(|app_bundle| {
        let bundle_name = app_bundle
            .file_name()
            .ok_or_else(|| "dmg app bundle has no directory name".to_string())?;
        copy_dir_recursive(&app_bundle, &staging_dir.join(bundle_name))
    });

    let detach_result = Command::new("hdiutil")
        .arg("detach")
        .arg(&mount_dir)
        .arg("-quiet")
        .status()
        .map_err(|error| format!("failed to detach dmg: {error}"));
    let _ = fs::remove_dir_all(&mount_dir);

    match (copy_result, detach_result) {
        (Ok(()), Ok(status)) if status.success() => Ok(()),
        (Ok(()), Ok(status)) => Err(format!("failed to detach dmg: {status}")),
        (Err(error), _) => Err(format!("failed to copy app bundle from dmg: {error}")),
        (Ok(()), Err(error)) => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn find_first_app_bundle(root: &Path) -> Result<PathBuf, String> {
    let entries =
        fs::read_dir(root).map_err(|error| format!("failed to read dmg mount dir: {error}"))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("failed to read dmg mount entry: {error}"))?
            .path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
            && path.is_dir()
        {
            return Ok(path);
        }
    }

    Err("dmg does not contain an app bundle".to_string())
}

/// 在 staging 目录内寻找解压后的完整客户端根目录。
pub fn find_staged_client_dir(staging_dir: &Path) -> Result<PathBuf, String> {
    if crate::client_scan::validate_client_dir(staging_dir)
        .is_ok_and(|client| client.health == crate::models::ClientHealth::Ok)
    {
        return Ok(staging_dir.to_path_buf());
    }

    let entries = fs::read_dir(staging_dir)
        .map_err(|error| format!("failed to read staging dir: {error}"))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("failed to read staging entry: {error}"))?
            .path();
        if path.is_dir()
            && crate::client_scan::validate_client_dir(&path)
                .is_ok_and(|client| client.health == crate::models::ClientHealth::Ok)
        {
            return Ok(path);
        }
    }

    Err("staging directory does not contain a healthy DDNet client".to_string())
}

/// 将 staging 中的客户端安装到目标目录，并为旧安装创建回滚目录。
pub fn install_staged_client(
    staged_client_dir: &Path,
    install_dir: &Path,
    rollback_dir: &Path,
) -> Result<(), String> {
    let replacement_dir = replacement_dir_for(install_dir);
    if replacement_dir.exists() {
        fs::remove_dir_all(&replacement_dir)
            .map_err(|error| format!("failed to clear replacement dir: {error}"))?;
    }
    if rollback_dir.exists() {
        fs::remove_dir_all(rollback_dir)
            .map_err(|error| format!("failed to clear rollback dir: {error}"))?;
    }
    if let Some(parent) = rollback_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create rollback parent: {error}"))?;
    }

    copy_dir_recursive(staged_client_dir, &replacement_dir)?;
    let replacement_client = crate::client_scan::validate_client_dir(&replacement_dir)?;
    if replacement_client.health != crate::models::ClientHealth::Ok {
        let _ = fs::remove_dir_all(&replacement_dir);
        return Err(format!(
            "replacement client is not healthy: {:?}",
            replacement_client.health
        ));
    }

    let had_existing_install = install_dir.exists();
    if had_existing_install {
        fs::rename(install_dir, rollback_dir)
            .map_err(|error| format!("failed to create rollback point: {error}"))?;
    }

    if let Err(error) = fs::rename(&replacement_dir, install_dir) {
        if had_existing_install && rollback_dir.exists() {
            if let Err(restore_error) = fs::rename(rollback_dir, install_dir) {
                return Err(format!(
                    "failed to activate replacement: {error}; failed to restore rollback: {restore_error}"
                ));
            }
        }
        return Err(format!("failed to activate replacement: {error}"));
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
pub fn restore_rollback(install_dir: &Path, rollback_dir: &Path) -> Result<(), String> {
    if !rollback_dir.exists() {
        return Err(format!(
            "rollback dir does not exist: {}",
            rollback_dir.display()
        ));
    }

    let failed_dir = failed_restore_dir_for(install_dir);
    if failed_dir.exists() {
        fs::remove_dir_all(&failed_dir)
            .map_err(|error| format!("failed to clear failed restore dir: {error}"))?;
    }

    let had_active_install = install_dir.exists();
    if had_active_install {
        fs::rename(install_dir, &failed_dir)
            .map_err(|error| format!("failed to move active install before rollback: {error}"))?;
    }

    if let Err(error) = fs::rename(rollback_dir, install_dir) {
        if had_active_install && failed_dir.exists() {
            if let Err(restore_error) = fs::rename(&failed_dir, install_dir) {
                return Err(format!(
                    "failed to restore rollback: {error}; failed to restore active install: {restore_error}"
                ));
            }
        }
        return Err(format!("failed to restore rollback: {error}"));
    }

    if failed_dir.exists() {
        fs::remove_dir_all(&failed_dir)
            .map_err(|error| format!("failed to clear replaced install after rollback: {error}"))?;
    }

    Ok(())
}

fn ensure_inside_root(root: &Path, path: &Path) -> Result<(), String> {
    let parent = path.parent().unwrap_or(root);
    let normalized_parent = normalize_existing_or_parent(parent)?;
    if normalized_parent.starts_with(root) {
        Ok(())
    } else {
        Err(format!("unsafe zip entry path: {}", path.display()))
    }
}

fn normalize_existing_or_parent(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return fs::canonicalize(path)
            .map_err(|error| format!("failed to canonicalize path: {error}"));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    normalize_existing_or_parent(parent)
}

fn copy_zip_entry<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> Result<(), String> {
    std::io::copy(reader, writer)
        .map(|_| ())
        .map_err(|error| format!("failed to extract zip entry: {error}"))
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("failed to create install dir: {error}"))?;

    for entry in
        fs::read_dir(source).map_err(|error| format!("failed to read source dir: {error}"))?
    {
        let entry = entry.map_err(|error| format!("failed to read source entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to read source file type: {error}"))?;
        if file_type.is_symlink() {
            copy_symlink(&source_path, &destination_path)?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| format!("failed to copy install file: {error}"))?;
        }
    }

    Ok(())
}

fn copy_symlink(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let target = fs::read_link(source_path)
            .map_err(|error| format!("failed to read install symlink: {error}"))?;
        std::os::unix::fs::symlink(&target, destination_path)
            .map_err(|error| format!("failed to copy install symlink: {error}"))
    }

    #[cfg(windows)]
    {
        let target = fs::read_link(source_path)
            .map_err(|error| format!("failed to read install symlink: {error}"))?;
        if source_path.is_dir() {
            std::os::windows::fs::symlink_dir(&target, destination_path)
                .map_err(|error| format!("failed to copy install symlink: {error}"))
        } else {
            std::os::windows::fs::symlink_file(&target, destination_path)
                .map_err(|error| format!("failed to copy install symlink: {error}"))
        }
    }
}

/// 校验下载 URL，并允许用户显式启用的代理或镜像 host。
pub(crate) fn validate_download_url_with_hosts(
    url: &str,
    enabled_route_hosts: &[String],
) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|error| format!("invalid download url: {error}"))?;
    if parsed.scheme() != "https" {
        return Err("download url must use https".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "download url must include host".to_string())?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return Err("download url host must be public".to_string());
    }
    let ip_host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = ip_host.parse::<IpAddr>() {
        validate_public_ip(ip)?;
    }
    if !TRUSTED_DOWNLOAD_HOSTS.contains(&host.as_str())
        && !enabled_route_hosts
            .iter()
            .any(|enabled| enabled.trim_end_matches('.').eq_ignore_ascii_case(&host))
    {
        return Err("download url host is not trusted".to_string());
    }
    Ok(())
}

async fn send_download_request(
    client: &reqwest::Client,
    asset_url: &str,
    enabled_route_hosts: &[String],
) -> Result<reqwest::Response, String> {
    let mut current_url =
        Url::parse(asset_url).map_err(|error| format!("invalid download url: {error}"))?;

    for _ in 0..=MAX_DOWNLOAD_REDIRECTS {
        validate_download_url_with_hosts(current_url.as_str(), enabled_route_hosts)?;
        let response = client
            .get(current_url.clone())
            .send()
            .await
            .map_err(|error| format!("failed to download update asset: {error}"))?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| "download redirect missing Location header".to_string())?
                .to_str()
                .map_err(|error| format!("download redirect Location is invalid: {error}"))?;
            current_url = current_url
                .join(location)
                .map_err(|error| format!("download redirect Location is invalid: {error}"))?;
            continue;
        }

        return response
            .error_for_status()
            .map_err(|error| format!("failed to download update asset: {error}"));
    }

    Err(format!(
        "download redirected more than {MAX_DOWNLOAD_REDIRECTS} times"
    ))
}

fn validate_public_ip(ip: IpAddr) -> Result<(), String> {
    let blocked = match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return validate_public_ip(IpAddr::V4(ipv4));
            }

            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || is_ipv6_unique_local(&ipv6)
                || is_ipv6_unicast_link_local(&ipv6)
        }
    };

    if blocked {
        return Err("download url host must be public".to_string());
    }

    Ok(())
}

fn is_ipv6_unique_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_ipv6_unicast_link_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn part_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download.zip");
    path.with_file_name(format!("{file_name}.part"))
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

impl PackageKind {
    /// 返回安装包类型用于持久化和诊断的稳定名称。
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Zip => "zip",
            PackageKind::TarXz => "tar.xz",
            PackageKind::Dmg => "dmg",
            PackageKind::Unknown => "unknown",
        }
    }

    fn cache_suffix(self) -> &'static str {
        match self {
            PackageKind::Zip => ".zip",
            PackageKind::TarXz => ".tar.xz",
            PackageKind::Dmg => ".dmg",
            PackageKind::Unknown => ".download",
        }
    }
}

#[cfg(test)]
#[path = "test/download.rs"]
mod tests;
