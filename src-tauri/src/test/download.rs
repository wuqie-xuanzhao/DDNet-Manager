//! download 模块测试入口：保留共享 helpers 和 download.rs 自身（PackageKind、
//! create_download_job、recover_from_registry）测试。
//!
//! 子模块按主题分文件存放：
//! - [`net`]：URL 校验与流式下载
//! - [`verify`]：SHA-256 校验与恢复摘要
//! - [`extract`]：zip / tar.xz / dmg 解压
//! - [`install`]：staged 替换与回滚

use crate::download::{
    auto_install_guard, create_download_job, package_kind_for_asset_url, DownloadManager,
    PackageKind,
};
use crate::models::{
    ClientUpdateCheck, DownloadJob, DownloadJobStatus, UpdateAction, UpdateAsset, UpdateSourceKind,
};
use crate::registry::ClientRegistry;
use std::fs;
use std::io::Write;

#[path = "download/extract.rs"]
mod extract;
#[path = "download/install.rs"]
mod install;
#[path = "download/net.rs"]
mod net;
#[path = "download/verify.rs"]
mod verify;

#[test]
fn create_download_job_uses_generated_cache_file_name() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update = sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip");
    let job = create_download_job("../evil", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert_eq!(cache_path.parent(), Some(temp_dir.path()));
    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("download-") && name.ends_with(".zip")));
}

#[test]
fn create_download_job_preserves_tar_xz_suffix_in_cache_path() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update =
        sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient.tar.xz");
    let job = create_download_job("../evil", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".tar.xz")));
}

#[test]
fn create_download_job_preserves_dmg_suffix_in_cache_path() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update = sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient.dmg");
    let job = create_download_job("../evil", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".dmg")));
}

#[test]
fn create_download_job_uses_download_suffix_for_unknown_asset_kind() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update =
        sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient.unknown");
    let job = create_download_job("../evil", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".download")));
}

#[test]
fn package_kind_for_asset_url_detects_supported_suffixes() {
    assert_eq!(
        package_kind_for_asset_url("https://example.com/QmClient.zip"),
        PackageKind::Zip
    );
    assert_eq!(
        package_kind_for_asset_url("https://example.com/QmClient.tar.xz"),
        PackageKind::TarXz
    );
    assert_eq!(
        package_kind_for_asset_url("https://example.com/QmClient.dmg"),
        PackageKind::Dmg
    );
    assert_eq!(
        package_kind_for_asset_url("https://example.com/QmClient.unknown"),
        PackageKind::Unknown
    );
}

#[test]
fn auto_install_guard_accepts_manager_owned_package_kinds() {
    assert!(auto_install_guard(PackageKind::Zip).is_ok());
    assert!(auto_install_guard(PackageKind::TarXz).is_ok());
    assert!(auto_install_guard(PackageKind::Dmg).is_ok());
    assert!(auto_install_guard(PackageKind::Unknown).is_err());
}

#[test]
fn recover_from_registry_transients_downloading_to_failed() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");

    let downloading =
        test_download_job_for_recovery("job-downloading", DownloadJobStatus::Downloading);
    registry
        .upsert_download_job(&downloading)
        .expect("下载中任务应写入注册表");

    let manager = DownloadManager::default();
    let recovered = manager
        .recover_from_registry(&registry)
        .expect("注册表恢复应成功");

    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, DownloadJobStatus::Failed);
    assert!(recovered[0].error.is_some());
    let in_memory = manager.get("job-downloading").expect("内存任务查询应成功");
    assert!(in_memory.is_some());
    assert_eq!(in_memory.unwrap().status, DownloadJobStatus::Failed);
}

#[test]
fn recover_from_registry_keeps_terminal_states_unchanged() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");

    for (id, status) in [
        ("job-verified", DownloadJobStatus::Verified),
        ("job-completed", DownloadJobStatus::Completed),
        ("job-canceled", DownloadJobStatus::Canceled),
        ("job-failed", DownloadJobStatus::Failed),
    ] {
        registry
            .upsert_download_job(&test_download_job_for_recovery(id, status.clone()))
            .expect("终端状态任务应写入注册表");
    }

    let manager = DownloadManager::default();
    let recovered = manager
        .recover_from_registry(&registry)
        .expect("注册表恢复应成功");

    assert_eq!(recovered.len(), 4);
    for job in &recovered {
        assert!(matches!(
            job.status,
            DownloadJobStatus::Verified
                | DownloadJobStatus::Completed
                | DownloadJobStatus::Canceled
                | DownloadJobStatus::Failed
        ));
        assert!(job.error.is_none() || job.status == DownloadJobStatus::Failed);
    }
}

// ===== 共享 helpers（被 extract / install 子模块测试使用） =====

pub(super) fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("测试 zip 文件应创建成功");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    for (name, bytes) in entries {
        zip.start_file(*name, options)
            .expect("测试 zip entry 应创建成功");
        zip.write_all(bytes).expect("测试 zip entry 内容应写入成功");
    }

    zip.finish().expect("测试 zip 应写入完成");
}

pub(super) fn write_tar_xz(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("测试 tar.xz 文件应创建成功");
    let encoder = xz2::write::XzEncoder::new(file, 6);
    let mut builder = tar::Builder::new(encoder);

    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        if name.contains("..") {
            append_raw_tar_entry(RawTarEntry {
                builder: &mut builder,
                header: &mut header,
                name,
                bytes,
            });
        } else {
            builder
                .append_data(&mut header, *name, *bytes)
                .expect("测试 tar entry 应写入成功");
        }
    }

    let encoder = builder.into_inner().expect("测试 tar 应写入完成");
    encoder.finish().expect("测试 xz 应写入完成");
}

struct RawTarEntry<'a, W: std::io::Write> {
    builder: &'a mut tar::Builder<W>,
    header: &'a mut tar::Header,
    name: &'a str,
    bytes: &'a [u8],
}

fn append_raw_tar_entry<W: std::io::Write>(entry: RawTarEntry<'_, W>) {
    let mut raw = *entry.header.as_bytes();
    let name_bytes = entry.name.as_bytes();
    raw[..name_bytes.len()].copy_from_slice(name_bytes);
    raw[148..156].fill(b' ');
    let checksum: u32 = raw.iter().map(|byte| u32::from(*byte)).sum();
    let checksum_text = format!("{checksum:06o}\0 ");
    raw[148..156].copy_from_slice(checksum_text.as_bytes());
    entry
        .builder
        .get_mut()
        .write_all(&raw)
        .expect("测试 tar raw header 应写入成功");
    entry
        .builder
        .get_mut()
        .write_all(entry.bytes)
        .expect("测试 tar raw body 应写入成功");
    let padding = (512 - (entry.bytes.len() % 512)) % 512;
    if padding > 0 {
        entry
            .builder
            .get_mut()
            .write_all(&vec![0; padding])
            .expect("测试 tar padding 应写入成功");
    }
}

pub(super) fn create_client_dir(path: &std::path::Path, executable_bytes: &[u8]) {
    fs::create_dir_all(path).expect("测试客户端目录应创建成功");
    fs::write(path.join("DDNet.exe"), executable_bytes).expect("测试可执行文件应写入成功");
    fs::write(path.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    fs::create_dir(path.join("data")).expect("测试 data 目录应创建成功");
}

pub(super) fn create_linux_client_dir(path: &std::path::Path, executable_bytes: &[u8]) {
    fs::create_dir_all(path).expect("测试 Linux 客户端目录应创建成功");
    fs::write(path.join("DDNet"), executable_bytes).expect("测试 Linux 可执行文件应写入成功");
    fs::write(path.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    fs::create_dir(path.join("data")).expect("测试 data 目录应创建成功");
}

pub(super) fn create_app_bundle(path: &std::path::Path, executable_bytes: &[u8]) {
    let macos_dir = path.join("Contents").join("MacOS");
    let resources_dir = path.join("Contents").join("Resources");
    fs::create_dir_all(&macos_dir).expect("测试 app bundle MacOS 目录应创建成功");
    fs::create_dir_all(&resources_dir).expect("测试 app bundle Resources 目录应创建成功");
    fs::write(macos_dir.join("DDNet"), executable_bytes).expect("测试 bundle 可执行文件应写入成功");
    fs::write(resources_dir.join("storage.cfg"), b"").expect("测试 bundle storage.cfg 应写入成功");
    fs::create_dir(resources_dir.join("data")).expect("测试 bundle data 目录应创建成功");
}

fn sample_update(asset_url: &str) -> ClientUpdateCheck {
    ClientUpdateCheck {
        client_id: "qmclient".to_string(),
        channel: "..\\bad".to_string(),
        current_version: None,
        latest_version: "C:/escape".to_string(),
        asset: UpdateAsset {
            platform: "windows-x86_64".to_string(),
            asset_url: asset_url.to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            size: 1,
        },
        needs_update: true,
        source_kind: UpdateSourceKind::Manifest,
        action: UpdateAction::Download,
        action_url: None,
        message: None,
    }
}

fn test_download_job_for_recovery(id: &str, status: DownloadJobStatus) -> DownloadJob {
    DownloadJob {
        id: id.to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        size: 2048,
        status,
        downloaded_bytes: 1024,
        cache_path: format!("C:/Cache/{id}.zip"),
        error: None,
    }
}
