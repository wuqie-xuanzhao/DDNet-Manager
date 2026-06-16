//! 压缩包安全解压到 staging 目录，处理 zip / tar.xz / dmg 三类安装包。

use super::PackageKind;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// 单个压缩包内最多允许的文件数，防止 zip bomb。
const MAX_ZIP_FILES: usize = 20_000;
/// 解压后字节数上限，防止磁盘被撑爆。
const MAX_UNPACKED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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
    let entry_size = entry
        .header()
        .size()
        .map_err(|error| format!("failed to read tar entry size: {error}"))?;
    current_size
        .checked_add(entry_size)
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
        super::install::copy_dir_recursive(&app_bundle, &staging_dir.join(bundle_name))
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
