#[cfg(test)]
mod tests {
    use crate::download::{
        auto_install_guard, create_download_job, download_asset_to_file, extract_zip_to_staging,
        install_staged_client, package_kind_for_asset_url, restore_rollback, rollback_dir_for,
        sha256_hex, validate_download_url_with_hosts, verify_downloaded_file, DownloadFileRequest,
        PackageKind,
    };
    use crate::models::{ClientUpdateCheck, UpdateAction, UpdateAsset, UpdateSourceKind};
    use std::fs;
    use std::io::Write;

    #[test]
    fn sha256_hex_matches_known_value() {
        assert_eq!(
            sha256_hex(b"ddnet-manager"),
            "739340afd53a209817636fca6d95d15abba5e236a11e49ff33e810111f00a55e"
        );
    }

    #[test]
    fn verify_downloaded_file_rejects_wrong_size_and_sha256() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let path = temp_dir.path().join("download.zip");
        fs::write(&path, b"ddnet-manager").expect("测试下载文件应写入成功");

        let wrong_size = verify_downloaded_file(&path, sha256_hex(b"ddnet-manager").as_str(), 1)
            .expect_err("错误 size 应被拒绝");
        let wrong_sha = verify_downloaded_file(
            &path,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            13,
        )
        .expect_err("错误 sha256 应被拒绝");

        assert!(wrong_size.contains("download size mismatch"));
        assert!(wrong_sha.contains("download sha256 mismatch"));
    }

    #[test]
    fn create_download_job_uses_generated_cache_file_name() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let update = ClientUpdateCheck {
            client_id: "qmclient".to_string(),
            channel: "..\\bad".to_string(),
            current_version: None,
            latest_version: "C:/escape".to_string(),
            asset: UpdateAsset {
                platform: "windows-x86_64".to_string(),
                asset_url: "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip"
                    .to_string(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                size: 1,
            },
            needs_update: true,
            source_kind: UpdateSourceKind::Manifest,
            action: UpdateAction::Download,
            action_url: None,
            message: None,
        };

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
        let update = sample_update(
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-linux.tar.xz",
        );

        let job = create_download_job("qmclient-linux", &update, temp_dir.path());
        let cache_path = std::path::PathBuf::from(job.cache_path);

        assert_eq!(cache_path.parent(), Some(temp_dir.path()));
        assert!(cache_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("download-") && name.ends_with(".tar.xz")));
    }

    #[test]
    fn create_download_job_preserves_dmg_suffix_in_cache_path() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let update =
            sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient-macos.dmg");

        let job = create_download_job("qmclient-macos", &update, temp_dir.path());
        let cache_path = std::path::PathBuf::from(job.cache_path);

        assert_eq!(cache_path.parent(), Some(temp_dir.path()));
        assert!(cache_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("download-") && name.ends_with(".dmg")));
    }

    #[test]
    fn create_download_job_uses_download_suffix_for_unknown_asset_kind() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let update = sample_update(
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-installer.7z",
        );

        let job = create_download_job("qmclient-unknown", &update, temp_dir.path());
        let cache_path = std::path::PathBuf::from(job.cache_path);

        assert_eq!(cache_path.parent(), Some(temp_dir.path()));
        assert!(cache_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("download-") && name.ends_with(".download")));
    }

    #[test]
    fn package_kind_for_asset_url_detects_supported_suffixes() {
        assert_eq!(
            package_kind_for_asset_url(
                "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip"
            ),
            PackageKind::Zip
        );
        assert_eq!(
            package_kind_for_asset_url(
                "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-linux.tar.xz"
            ),
            PackageKind::TarXz
        );
        assert_eq!(
            package_kind_for_asset_url(
                "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-macos.dmg"
            ),
            PackageKind::Dmg
        );
        assert_eq!(
            package_kind_for_asset_url(
                "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-installer.7z"
            ),
            PackageKind::Unknown
        );
    }

    #[test]
    fn auto_install_guard_rejects_tar_xz_and_dmg_packages() {
        let tar_xz_error =
            auto_install_guard(PackageKind::TarXz).expect_err("当前自动安装流程不应接受 tar.xz 包");
        let dmg_error =
            auto_install_guard(PackageKind::Dmg).expect_err("当前自动安装流程不应接受 dmg 包");

        assert_eq!(
            tar_xz_error,
            "automatic install only supports .zip packages; .tar.xz requires manual install for now"
        );
        assert_eq!(
            dmg_error,
            "automatic install only supports .zip packages; .dmg requires manual install for now"
        );
    }

    #[test]
    fn extract_zip_to_staging_extracts_safe_zip() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let zip_path = temp_dir.path().join("safe.zip");
        let staging_dir = temp_dir.path().join("staging");
        write_zip(&zip_path, &[("QmClient/DDNet.exe", b"exe".as_slice())]);

        extract_zip_to_staging(&zip_path, &staging_dir).expect("安全 zip 应解压成功");

        assert_eq!(
            fs::read(staging_dir.join("QmClient").join("DDNet.exe")).expect("解压文件应可读取"),
            b"exe"
        );
    }

    #[test]
    fn extract_zip_to_staging_rejects_path_traversal() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let zip_path = temp_dir.path().join("evil.zip");
        let staging_dir = temp_dir.path().join("staging");
        write_zip(&zip_path, &[("../evil.txt", b"nope".as_slice())]);

        let error = extract_zip_to_staging(&zip_path, &staging_dir).expect_err("路径穿越应被拒绝");

        assert!(error.contains("unsafe zip entry path"));
        assert!(!temp_dir.path().join("evil.txt").exists());
    }

    #[tokio::test]
    async fn download_asset_to_file_rejects_private_hosts_before_network() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let cache_path = temp_dir.path().join("download.zip");

        for url in [
            "https://localhost/file.zip",
            "https://127.0.0.1/file.zip",
            "https://10.0.0.1/file.zip",
            "https://169.254.1.1/file.zip",
            "https://[::1]/file.zip",
            "https://[fc00::1]/file.zip",
            "https://[::ffff:127.0.0.1]/file.zip",
        ] {
            let error = download_asset_to_file(
                DownloadFileRequest {
                    asset_url: url,
                    cache_path: &cache_path,
                    expected_size: 1,
                    enabled_route_hosts: &[],
                },
                |_| true,
            )
            .await
            .expect_err("私网或本机下载地址应被拒绝");

            assert_eq!(error, "download url host must be public");
        }
    }

    #[tokio::test]
    async fn download_asset_to_file_rejects_untrusted_public_host_before_network() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let cache_path = temp_dir.path().join("download.zip");

        let error = download_asset_to_file(
            DownloadFileRequest {
                asset_url: "https://example.com/file.zip",
                cache_path: &cache_path,
                expected_size: 1,
                enabled_route_hosts: &[],
            },
            |_| true,
        )
        .await
        .expect_err("非可信 host 应被拒绝");

        assert_eq!(error, "download url host is not trusted");
    }

    #[test]
    fn validate_download_url_accepts_enabled_route_host() {
        let enabled_hosts = vec!["proxy.invalid".to_string()];

        validate_download_url_with_hosts(
            "https://proxy.invalid/proxy/https%3A%2F%2Fgithub.com%2Fddnet%2Fddnet%2Freleases%2Fdownload%2Fv1%2Fqmclient.zip",
            &enabled_hosts,
        )
        .expect("显式启用的代理 host 应可用于下载");
    }

    #[test]
    fn validate_download_url_rejects_ambiguous_numeric_hosts_even_when_enabled() {
        for host in ["127.1", "2130706433", "0177.0.0.1"] {
            let enabled_hosts = vec![host.to_string()];
            let url = format!("https://{host}/file.zip");

            let error = validate_download_url_with_hosts(&url, &enabled_hosts)
                .expect_err("歧义数字 host 即使显式启用也应拒绝");

            assert_eq!(error, "download url host must be public", "{host}");
        }
    }

    #[test]
    fn validate_download_url_accepts_github_release_redirect_host() {
        validate_download_url_with_hosts(
            "https://release-assets.githubusercontent.com/github-production-release-asset/example.zip",
            &[],
        )
        .expect("GitHub Release 资产重定向 host 应可用于直连下载");
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

    fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
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

    fn create_client_dir(path: &std::path::Path, executable_bytes: &[u8]) {
        fs::create_dir_all(path).expect("测试客户端目录应创建成功");
        fs::write(path.join("DDNet.exe"), executable_bytes).expect("测试可执行文件应写入成功");
        fs::write(path.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        fs::create_dir(path.join("data")).expect("测试 data 目录应创建成功");
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
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                size: 1,
            },
            needs_update: true,
            source_kind: UpdateSourceKind::Manifest,
            action: UpdateAction::Download,
            action_url: None,
            message: None,
        }
    }
}
