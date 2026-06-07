#[cfg(test)]
mod tests {
    use crate::client_scan as scan;
    use crate::models::ClientInstallSource;

    #[test]
    fn validate_client_dir_returns_ok_for_complete_directory() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = scan::validate_client_dir(install_dir).expect("完整目录应验证成功");

        assert_eq!(installation.health, crate::models::ClientHealth::Ok);
        assert!(installation.executable_path.ends_with("DDNet.exe"));
    }

    #[test]
    fn validate_client_dir_classifies_unknown_complete_directory_as_third_party() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path().join("CustomClient");
        std::fs::create_dir(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = scan::validate_client_dir(&install_dir).expect("完整目录应验证成功");

        assert_eq!(installation.client_id, "third_party");
        assert_eq!(installation.display_name, "CustomClient");
        assert_eq!(installation.health, crate::models::ClientHealth::Ok);
        assert!(installation.last_scanned_at.is_some());
    }

    #[test]
    fn validate_client_dir_classifies_qmclient_directory_by_name() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path().join("QmClient");
        std::fs::create_dir(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = scan::validate_client_dir(&install_dir).expect("完整目录应验证成功");

        assert_eq!(installation.client_id, "qmclient");
        assert_eq!(installation.display_name, "QmClient");
    }

    #[test]
    fn validate_client_dir_uses_ddnet_client_id_for_official_directory() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path().join("DDNet");
        std::fs::create_dir(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("DDNet.exe"), b"MZ").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = scan::validate_client_dir(&install_dir).expect("完整目录应验证成功");

        assert_eq!(installation.client_id, "ddnet");
        assert_ne!(installation.client_id, "ddnet_vanilla");
        assert_eq!(installation.display_name, "DDNet");
    }

    #[test]
    fn validate_client_dir_detects_steam_ddnet_source() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir
            .path()
            .join("SteamLibrary")
            .join("steamapps")
            .join("common")
            .join("DDNet");
        std::fs::create_dir_all(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("DDNet.exe"), b"MZ").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = scan::validate_client_dir(&install_dir).expect("完整目录应验证成功");

        assert_eq!(installation.client_id, "ddnet");
        assert_eq!(installation.install_source, ClientInstallSource::Steam);
        assert_eq!(
            installation.upstream_url.as_deref(),
            Some("https://store.steampowered.com/app/412220/DDNet/")
        );
    }

    #[test]
    fn validate_client_dir_accepts_macos_app_bundle() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let app_dir = temp_dir.path().join("QmClient.app");
        std::fs::create_dir_all(app_dir.join("Contents").join("MacOS"))
            .expect("测试 MacOS 目录应创建成功");
        std::fs::create_dir_all(app_dir.join("Contents").join("Resources").join("data"))
            .expect("测试 data 目录应创建成功");
        std::fs::write(
            app_dir.join("Contents").join("MacOS").join("DDNet"),
            b"mach-o",
        )
        .expect("测试 bundle 可执行文件应写入成功");
        std::fs::write(app_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");

        let installation = scan::validate_client_dir(&app_dir).expect(".app bundle 应验证成功");

        assert_eq!(installation.client_id, "qmclient");
        assert!(installation
            .executable_path
            .ends_with("QmClient.app/Contents/MacOS/DDNet"));
        assert!(installation
            .data_dir
            .ends_with("QmClient.app/Contents/Resources/data"));
    }

    #[test]
    fn validate_client_dir_accepts_linux_ddnet_executable_name() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path().join("TClient-linux");
        std::fs::create_dir(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("DDNet"), b"elf").expect("测试 Linux 可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation = scan::validate_client_dir(&install_dir).expect("Linux 目录应验证成功");

        assert_eq!(installation.client_id, "taterclient");
        assert!(installation
            .executable_path
            .ends_with("TClient-linux/DDNet"));
    }

    #[test]
    fn scan_client_installations_finds_candidates_under_roots() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path().join("Games").join("QmClient");
        std::fs::create_dir_all(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let options = scan::ScanOptions {
            roots: vec![temp_dir.path().to_path_buf()],
            include_saved_paths: false,
            deep: false,
            use_everything: false,
        };
        let installations = scan::scan_client_installations(&options).expect("扫描应成功");

        assert_eq!(installations.len(), 1);
        assert_eq!(installations[0].health, crate::models::ClientHealth::Ok);
        assert_eq!(installations[0].client_id, "qmclient");
    }

    #[test]
    fn scan_client_installations_ignores_data_only_directories() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let data_only_dir = temp_dir.path().join("NotAClient");
        std::fs::create_dir_all(data_only_dir.join("data")).expect("测试 data 目录应创建成功");

        let options = scan::ScanOptions {
            roots: vec![temp_dir.path().to_path_buf()],
            include_saved_paths: false,
            deep: false,
            use_everything: false,
        };
        let installations = scan::scan_client_installations(&options).expect("扫描应成功");

        assert!(installations.is_empty());
    }

    #[test]
    fn steam_libraryfolders_text_returns_ddnet_roots() {
        let roots = scan::steam_ddnet_roots_from_libraryfolders_text(
            r#"
                "libraryfolders"
                {
                    "0"
                    {
                        "path" "C:\\Program Files (x86)\\Steam"
                    }
                    "1"
                    {
                        "path" "D:\\SteamLibrary"
                    }
                }
                "#,
        );

        assert_eq!(
            roots,
            vec![
                std::path::Path::new("C:\\Program Files (x86)\\Steam")
                    .join("steamapps")
                    .join("common")
                    .join("DDNet"),
                std::path::Path::new("D:\\SteamLibrary")
                    .join("steamapps")
                    .join("common")
                    .join("DDNet")
            ]
        );
    }

    #[test]
    fn common_scan_roots_include_user_appdata_program_and_steam_locations() {
        let roots = scan::common_scan_roots_from_env_values(scan::CommonScanRootEnv {
            user_profile: Some(std::path::Path::new("C:/Users/Player")),
            program_files: Some(std::path::Path::new("C:/Program Files")),
            program_files_x86: Some(std::path::Path::new("C:/Program Files (x86)")),
            local_appdata: Some(std::path::Path::new("C:/Users/Player/AppData/Local")),
            appdata: Some(std::path::Path::new("C:/Users/Player/AppData/Roaming")),
            program_data: Some(std::path::Path::new("C:/ProgramData")),
        });

        assert!(roots.contains(&std::path::Path::new("C:/Users/Player/Downloads").to_path_buf()));
        assert!(roots.contains(&std::path::Path::new("C:/Users/Player/Desktop").to_path_buf()));
        assert!(roots.contains(&std::path::Path::new("C:/Users/Player/Documents").to_path_buf()));
        assert!(roots.contains(&std::path::Path::new("C:/Users/Player/Games").to_path_buf()));
        assert!(
            roots.contains(&std::path::Path::new("C:/Users/Player/AppData/Local").to_path_buf())
        );
        assert!(
            roots.contains(&std::path::Path::new("C:/Users/Player/AppData/Roaming").to_path_buf())
        );
        assert!(roots.contains(
            &std::path::Path::new("C:/ProgramData/Microsoft/Windows/Start Menu/Programs")
                .to_path_buf()
        ));
        assert!(roots.contains(
            &std::path::Path::new("F:/SteamLibrary/steamapps/common/DDNet").to_path_buf()
        ));
    }

    #[test]
    fn scan_client_installations_finds_lowercase_ddnet_executable() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path().join("DDNet");
        std::fs::create_dir(&install_dir).expect("测试安装目录应创建成功");
        std::fs::write(install_dir.join("ddnet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let options = scan::ScanOptions {
            roots: vec![temp_dir.path().to_path_buf()],
            include_saved_paths: false,
            deep: false,
            use_everything: false,
        };
        let installations = scan::scan_client_installations(&options).expect("扫描应成功");

        assert_eq!(installations.len(), 1);
        assert_eq!(installations[0].health, crate::models::ClientHealth::Ok);
        assert!(installations[0].executable_path.ends_with("ddnet.exe"));
    }

    #[test]
    fn everything_output_returns_parent_candidate_dirs() {
        let output = "D:\\Games\\QmClient\\DDNet.exe\nE:\\DDNet\\storage.cfg\n";

        let candidates = scan::everything_candidate_dirs_from_output(output);

        assert_eq!(
            candidates,
            vec![
                std::path::Path::new("D:\\Games\\QmClient").to_path_buf(),
                std::path::Path::new("E:\\DDNet").to_path_buf()
            ]
        );
    }

    #[test]
    fn everything_output_deduplicates_candidate_dirs() {
        let output = "D:\\DDNet\\DDNet.exe\nD:\\DDNet\\storage.cfg\n";

        let candidates = scan::everything_candidate_dirs_from_output(output);

        assert_eq!(
            candidates,
            vec![std::path::Path::new("D:\\DDNet").to_path_buf()]
        );
    }

    #[test]
    fn everything_executable_candidates_do_not_use_path_lookup() {
        let candidates = scan::everything_executable_candidates();

        assert!(!candidates.iter().any(|path| path == "es.exe"));
    }

    #[test]
    fn validate_client_dir_reports_missing_executable() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation =
            scan::validate_client_dir(install_dir).expect("缺少可执行文件也应返回安装记录");

        assert_eq!(
            installation.health,
            crate::models::ClientHealth::MissingExecutable
        );
    }

    #[test]
    fn validate_client_dir_reports_missing_storage_cfg() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let installation =
            scan::validate_client_dir(install_dir).expect("缺少 storage.cfg 也应返回安装记录");

        assert_eq!(
            installation.health,
            crate::models::ClientHealth::MissingStorageCfg
        );
    }

    #[test]
    fn validate_client_dir_reports_missing_data_dir() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");

        let installation =
            scan::validate_client_dir(install_dir).expect("缺少 data 也应返回安装记录");

        assert_eq!(
            installation.health,
            crate::models::ClientHealth::MissingDataDir
        );
    }

    #[test]
    fn validate_client_dir_rejects_non_directory_input() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let file_path = temp_dir.path().join("DDNet.exe");
        std::fs::write(&file_path, b"").expect("测试文件应写入成功");

        let result = scan::validate_client_dir(&file_path);

        assert!(result.is_err());
    }

    #[test]
    fn validate_client_dir_uses_canonical_path_for_stable_id() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let install_dir = temp_dir.path();
        std::fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");
        std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
        std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

        let direct = scan::validate_client_dir(install_dir).expect("直接路径应验证成功");
        let dotted = scan::validate_client_dir(&install_dir.join(".")).expect("等价路径应验证成功");

        assert_eq!(direct.id, dotted.id);
    }

    #[test]
    fn normalize_path_replaces_backslashes_with_forward_slashes() {
        assert_eq!(
            scan::normalize_path(std::path::Path::new(r"C:\Games\QmClient\DDNet.exe")),
            "C:/Games/QmClient/DDNet.exe"
        );
    }

    #[test]
    fn stable_installation_id_uses_fixed_fnv1a64_value() {
        assert_eq!(
            scan::stable_installation_id("qmclient", "C:/Games/QmClient"),
            "qmclient-0010748f07d5a3e0"
        );
    }
}
