//! 安装事务（staged 替换、回滚、恢复）的测试。

use super::{create_app_bundle, create_client_dir, create_linux_client_dir, write_tar_xz};
use crate::download::install::{install_staged_client, restore_rollback, rollback_dir_for};
use crate::download::{extract_package_to_staging, find_staged_client_dir, PackageKind};
use std::fs;

#[test]
fn tar_xz_staging_can_install_with_rollback() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let archive_path = temp_dir.path().join("client.tar.xz");
    let staging_dir = temp_dir.path().join("staging");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("rollback");
    write_tar_xz(
        &archive_path,
        &[
            ("QmClient/DDNet", b"new".as_slice()),
            ("QmClient/storage.cfg", b"".as_slice()),
            ("QmClient/data/.keep", b"".as_slice()),
        ],
    );
    create_linux_client_dir(&install_dir, b"old");

    extract_package_to_staging(&archive_path, &staging_dir, PackageKind::TarXz)
        .expect("tar.xz 应安全解包到 staging");
    let staged =
        crate::download::find_staged_client_dir(&staging_dir).expect("staging 应包含健康客户端");
    install_staged_client(&staged, &install_dir, &rollback_dir).expect("tar.xz staging 应可安装");

    assert_eq!(
        fs::read(install_dir.join("DDNet")).expect("新 Linux 可执行文件应存在"),
        b"new"
    );
    assert_eq!(
        fs::read(rollback_dir.join("DDNet")).expect("旧安装应进入回滚目录"),
        b"old"
    );
}

#[test]
fn install_staged_app_bundle_preserves_app_directory_name() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staged_app = temp_dir.path().join("DDNet-Official.app");
    let install_app = temp_dir.path().join("DDNet-Manager.app");
    let rollback_app = temp_dir.path().join("DDNet-Manager.rollback.app");
    create_app_bundle(&staged_app, b"new");
    create_app_bundle(&install_app, b"old");

    install_staged_client(&staged_app, &install_app, &rollback_app)
        .expect(".app bundle 应可作为 Manager-owned 安装根");

    assert!(install_app
        .join("Contents")
        .join("MacOS")
        .join("DDNet")
        .exists());
    assert!(rollback_app
        .join("Contents")
        .join("MacOS")
        .join("DDNet")
        .exists());
    assert!(crate::client_scan::validate_client_dir(&install_app).is_ok());
}

#[test]
fn install_staged_client_creates_rollback_and_activates_replacement() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staged_dir = temp_dir.path().join("staged");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("rollback");
    create_client_dir(&staged_dir, b"new");
    create_client_dir(&install_dir, b"old");

    install_staged_client(&staged_dir, &install_dir, &rollback_dir).expect("安装应成功");

    assert_eq!(
        fs::read(install_dir.join("DDNet.exe")).expect("新安装应存在"),
        b"new"
    );
    assert_eq!(
        fs::read(rollback_dir.join("DDNet.exe")).expect("回滚点应存在"),
        b"old"
    );
}

#[test]
fn install_staged_client_keeps_existing_install_when_staging_is_unhealthy() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staged_dir = temp_dir.path().join("staged");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("rollback");
    fs::create_dir(&staged_dir).expect("测试 staging 目录应创建成功");
    fs::write(staged_dir.join("DDNet.exe"), b"new").expect("测试 staging 文件应写入成功");
    create_client_dir(&install_dir, b"old");

    let error = install_staged_client(&staged_dir, &install_dir, &rollback_dir)
        .expect_err("不健康 staging 应失败");

    assert!(error.contains("replacement client is not healthy"));
    assert_eq!(
        fs::read(install_dir.join("DDNet.exe")).expect("旧安装应保留"),
        b"old"
    );
    assert!(!rollback_dir.exists());
}

#[test]
fn rollback_dir_for_uses_install_parent_to_avoid_cross_volume_rename() {
    let install_dir = std::path::Path::new("D:/Games/QmClient");
    let rollback_dir = rollback_dir_for(install_dir, "install-download-1");

    assert_eq!(
        rollback_dir,
        std::path::Path::new("D:/Games/QmClient.ddnet-manager-rollback-install-download-1")
            .to_path_buf()
    );
}

#[test]
fn restore_rollback_replaces_active_install_with_rollback() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("QmClient.rollback");
    create_client_dir(&install_dir, b"new");
    create_client_dir(&rollback_dir, b"old");

    restore_rollback(&install_dir, &rollback_dir).expect("回滚应恢复旧客户端");

    assert_eq!(
        fs::read(install_dir.join("DDNet.exe")).expect("恢复后的客户端应存在"),
        b"old"
    );
    assert!(!rollback_dir.exists());
}

#[test]
fn find_staged_client_dir_returns_root_when_root_is_healthy() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staging_root = temp_dir.path().join("staging");
    create_linux_client_dir(&staging_root, b"exe");

    let discovered = find_staged_client_dir(&staging_root).expect("staging 根应能直接识别为客户端");

    assert_eq!(discovered, staging_root);
}
