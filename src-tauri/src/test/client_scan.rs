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

    let installation = scan::validate_client_dir(install_dir).expect("缺少 data 也应返回安装记录");

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
fn normalize_for_compare_trims_trailing_slash_and_lowercases_on_windows() {
    // Windows：大小写不敏感 + 去尾斜杠
    let expected = if cfg!(windows) {
        "c:/games/qmclient"
    } else {
        "C:/Games/QmClient"
    };
    assert_eq!(
        scan::normalize_for_compare(std::path::Path::new(r"C:\Games\QmClient\")),
        expected
    );
    assert_eq!(
        scan::normalize_for_compare(std::path::Path::new("C:/Games/QmClient/")),
        expected
    );
}

#[test]
fn stable_installation_id_uses_fixed_fnv1a64_value() {
    assert_eq!(
        scan::stable_installation_id("qmclient", "C:/Games/QmClient"),
        "qmclient-0010748f07d5a3e0"
    );
}

#[test]
fn infer_client_identity_matches_qmclient_path() {
    let identity =
        scan::infer_client_identity(std::path::Path::new("D:/Games/QmClient"), None, None, None);
    assert_eq!(identity.client_id, "qmclient");
    assert_eq!(identity.display_name, "QmClient");
    assert_eq!(identity.install_source, ClientInstallSource::Manual);
    assert!(identity.upstream_url.is_some());
}

#[test]
fn infer_client_identity_marks_steam_ddnet_as_steam_source() {
    let identity = scan::infer_client_identity(
        std::path::Path::new("C:/Steam/steamapps/common/ddnet"),
        None,
        None,
        None,
    );
    assert_eq!(identity.client_id, "ddnet");
    assert_eq!(identity.display_name, "DDNet");
    assert_eq!(identity.install_source, ClientInstallSource::Steam);
}

#[test]
fn infer_client_identity_falls_back_to_third_party_for_unknown_path() {
    let identity = scan::infer_client_identity(
        std::path::Path::new("D:/Games/random-game"),
        None,
        None,
        None,
    );
    assert_eq!(identity.client_id, "third_party");
    assert_eq!(identity.display_name, "random-game");
    assert_eq!(identity.install_source, ClientInstallSource::Manual);
    assert!(identity.upstream_url.is_none());
}

// ===== PE 元信息优先级测试 =====

#[test]
fn infer_client_identity_falls_back_to_path_when_pe_indistinguishable() {
    // 真实场景：PE 元信息是 "DDNet Team"/"DDNet"（5 个 entry 都匹配，match_catalog_by_pe
    // 返回 None），路径含 "tclient" 子串 → 走路径匹配识别为 taterclient。
    // 历史反例：旧逻辑会把任何 PE="DDNet Team" 的客户端识别为 ddnet 原版，导致
    // QmClient/TaterClient/BestClient/Cactus 全部误判。
    let identity = scan::infer_client_identity(
        std::path::Path::new("D:/Games/tclient-fork/bestclient"),
        Some("DDNet Team"),
        Some("DDNet"),
        None,
    );
    // 路径 "tclient" 命中 taterclient alias（顺序优先于 "bestclient"，因为 taterclient
    // 在 CATALOG 中先定义），且 PE 不可区分不再覆盖路径匹配。
    assert_eq!(
        identity.client_id, "taterclient",
        "PE 不可区分时应 fallback 到路径匹配"
    );
}

#[test]
fn infer_client_identity_returns_third_party_when_pe_indistinguishable_and_path_misses() {
    // 路径无任何 alias 命中，PE 元信息 5 个 entry 都匹配（"DDNet Team"/"DDNet"）
    // → match_catalog_by_pe 返回 None → 路径无命中 → third_party fallback。
    // 历史反例：旧逻辑会把这种场景识别为 ddnet，错误归属。
    let identity = scan::infer_client_identity(
        std::path::Path::new("D:/MyStuff/random-folder"),
        Some("DDNet Team"),
        Some("DDNet"),
        None,
    );
    assert_eq!(identity.client_id, "third_party");
}

#[test]
fn infer_client_identity_falls_back_to_path_when_pe_does_not_match() {
    // PE 元信息不匹配任何 catalog entry（第三方发行方），应 fallback 到路径匹配
    let identity = scan::infer_client_identity(
        std::path::Path::new("D:/Games/QmClient"),
        Some("Unknown Studio"),
        Some("Custom Fork"),
        None,
    );
    assert_eq!(
        identity.client_id, "qmclient",
        "PE 不匹配时应 fallback 到路径匹配"
    );
}

#[test]
fn infer_client_identity_falls_back_to_third_party_when_both_pe_and_path_miss() {
    let identity = scan::infer_client_identity(
        std::path::Path::new("D:/Games/random-shooter"),
        Some("Unknown Studio"),
        Some("Some Game"),
        None,
    );
    assert_eq!(identity.client_id, "third_party");
}

#[test]
fn infer_client_identity_steam_path_overrides_pe_match() {
    // Steam DDNet 路径权威，即使 PE 元信息显示是第三方 fork，也强制识别为 ddnet
    let identity = scan::infer_client_identity(
        std::path::Path::new("C:/Steam/steamapps/common/ddnet"),
        Some("Custom Studio"),
        Some("Custom Fork"),
        None,
    );
    assert_eq!(identity.client_id, "ddnet");
    assert_eq!(identity.install_source, ClientInstallSource::Steam);
}

// ===== 兼容性检测测试 =====

/// 构造一个最小有效 PE 文件，仅包含 DOS header + PE signature + COFF Machine 字段。
fn make_test_pe(machine: u16) -> Vec<u8> {
    let mut data = vec![0u8; 128];

    // DOS header magic
    data[0] = b'M';
    data[1] = b'Z';

    // PE offset at 0x3C
    let pe_offset = 0x40u32;
    data[0x3C..0x40].copy_from_slice(&pe_offset.to_le_bytes());

    // PE signature
    data[0x40] = b'P';
    data[0x41] = b'E';
    data[0x42] = 0;
    data[0x43] = 0;

    // COFF header machine field
    data[0x44..0x46].copy_from_slice(&machine.to_le_bytes());

    data
}

#[test]
fn pe_architecture_parses_x86_pe() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let path = temp_dir.path().join("test_x86.exe");
    std::fs::write(&path, make_test_pe(0x014c)).expect("测试 PE 应写入成功");
    assert_eq!(scan::pe_architecture(&path), Some("x86"));
}

#[test]
fn pe_architecture_parses_x64_pe() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let path = temp_dir.path().join("test_x64.exe");
    std::fs::write(&path, make_test_pe(0x8664)).expect("测试 PE 应写入成功");
    assert_eq!(scan::pe_architecture(&path), Some("x86_64"));
}

#[test]
fn pe_architecture_returns_none_for_non_pe() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let path = temp_dir.path().join("not_pe.txt");
    std::fs::write(&path, b"hello world").expect("测试文件应写入成功");
    assert_eq!(scan::pe_architecture(&path), None);
}

#[test]
fn pe_architecture_returns_none_for_mz_without_valid_pe() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let path = temp_dir.path().join("broken.exe");
    let mut data = vec![0u8; 128];
    data[0] = b'M';
    data[1] = b'Z';
    // 不写入有效的 PE offset，导致解析失败
    std::fs::write(&path, data).expect("测试文件应写入成功");
    assert_eq!(scan::pe_architecture(&path), None);
}

#[test]
fn detect_client_compatibility_returns_supported_for_valid_directory() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path();
    std::fs::write(install_dir.join("DDNet.exe"), make_test_pe(0x8664))
        .expect("测试可执行文件应写入成功");
    std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

    let installation = scan::validate_client_dir(install_dir).expect("完整目录应验证成功");

    assert_eq!(
        installation.compatibility.status,
        crate::models::CompatibilityStatus::Supported
    );
    assert!(installation.compatibility.can_launch);
    assert!(!installation.compatibility.launch_verified);
    assert!(installation.compatibility.reasons.is_empty());
}
