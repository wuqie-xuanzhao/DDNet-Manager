use super::{
    BindConflict, CfgAnalysis, CfgExecRecord, CfgUnbindRecord, ClientCompatibility,
    ClientConfidence, ClientHealth, ClientInstallSource, ClientInstallation, CompatibilityStatus,
    WorkshopBind,
};

#[test]
fn serializes_client_installation_with_snake_case_fields_and_ok_health() {
    let installation = ClientInstallation {
        id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir: "D:/Games/QmClient".to_string(),
        executable_path: "D:/Games/QmClient/DDNet.exe".to_string(),
        storage_cfg_path: "D:/Games/QmClient/storage.cfg".to_string(),
        data_dir: "D:/Games/QmClient/data".to_string(),
        user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
        version: Some("18.9.1".to_string()),
        is_default: true,
        health: ClientHealth::Ok,
        missing_items: Vec::new(),
        install_source: ClientInstallSource::Manual,
        confidence: ClientConfidence::Compatible,
        manager_owned: false,
        compatibility: ClientCompatibility {
            status: CompatibilityStatus::Unknown,
            can_launch: true,
            launch_verified: false,
            reasons: Vec::new(),
            last_launch_result: None,
        },
        upstream_url: None,
        last_scanned_at: Some("2026-06-06T12:00:00Z".to_string()),
    };

    let serialized = serde_json::to_value(installation).expect("测试序列化应成功");

    assert_eq!(serialized["client_id"], "qmclient");
    assert_eq!(serialized["display_name"], "QmClient");
    assert_eq!(serialized["install_dir"], "D:/Games/QmClient");
    assert_eq!(serialized["executable_path"], "D:/Games/QmClient/DDNet.exe");
    assert_eq!(
        serialized["storage_cfg_path"],
        "D:/Games/QmClient/storage.cfg"
    );
    assert_eq!(serialized["data_dir"], "D:/Games/QmClient/data");
    assert_eq!(
        serialized["user_data_dir"],
        "C:/Users/Player/AppData/Roaming/DDNet"
    );
    assert_eq!(serialized["is_default"], true);
    assert_eq!(serialized["health"], "ok");
}

#[test]
fn serializes_client_installation_with_mvp_metadata() {
    let installation = ClientInstallation {
        id: "ddnet-main".to_string(),
        client_id: "ddnet".to_string(),
        display_name: "DDNet".to_string(),
        install_dir: "D:/SteamLibrary/steamapps/common/DDNet".to_string(),
        executable_path: "D:/SteamLibrary/steamapps/common/DDNet/DDNet.exe".to_string(),
        storage_cfg_path: "D:/SteamLibrary/steamapps/common/DDNet/storage.cfg".to_string(),
        data_dir: "D:/SteamLibrary/steamapps/common/DDNet/data".to_string(),
        user_data_dir: None,
        version: Some("19.8.2".to_string()),
        is_default: true,
        health: ClientHealth::Ok,
        missing_items: Vec::new(),
        install_source: ClientInstallSource::Steam,
        confidence: ClientConfidence::Verified,
        manager_owned: false,
        compatibility: ClientCompatibility {
            status: CompatibilityStatus::Supported,
            can_launch: true,
            launch_verified: false,
            reasons: Vec::new(),
            last_launch_result: None,
        },
        upstream_url: Some("https://store.steampowered.com/app/412220/DDNet/".to_string()),
        last_scanned_at: Some("2026-06-07T12:00:00Z".to_string()),
    };

    let serialized = serde_json::to_value(installation).expect("测试序列化应成功");

    assert_eq!(serialized["client_id"], "ddnet");
    assert_eq!(serialized["install_source"], "steam");
    assert_eq!(serialized["confidence"], "verified");
    assert_eq!(serialized["manager_owned"], false);
    assert_eq!(serialized["compatibility"]["status"], "supported");
}

#[test]
fn serializes_workshop_bind_with_snake_case_fields() {
    let bind = WorkshopBind {
        id: "bind-1".to_string(),
        category: "基础".to_string(),
        title: "防自杀".to_string(),
        command: "bind mouse3 \"kill\"".to_string(),
        description: "测试".to_string(),
        command_variants: vec!["bind mouse3 \"echo Kill; kill\"".to_string()],
        variant_labels: vec!["带提示".to_string()],
        is_bindable: true,
    };

    let serialized = serde_json::to_value(bind).expect("测试序列化应成功");

    assert_eq!(
        serialized["command_variants"][0],
        "bind mouse3 \"echo Kill; kill\""
    );
    assert_eq!(serialized["variant_labels"][0], "带提示");
    assert_eq!(serialized["is_bindable"], true);
    assert!(serialized.get("commandVariants").is_none());
    assert!(serialized.get("variantLabels").is_none());
    assert!(serialized.get("isBindable").is_none());
}

#[test]
fn serializes_cfg_analysis_related_types_with_snake_case_fields() {
    let analysis = CfgAnalysis {
        binds: vec![],
        unbinds: vec![CfgUnbindRecord {
            key: "mouse3".to_string(),
            source_file: "settings_ddnet.cfg".to_string(),
            line: 4,
        }],
        execs: vec![CfgExecRecord {
            target: "extra.cfg".to_string(),
            source_file: "settings_ddnet.cfg".to_string(),
            line: 3,
            resolved_path: Some("C:/DDNet/extra.cfg".to_string()),
            missing: false,
        }],
        conflicts: vec![BindConflict {
            key: "f1".to_string(),
            records: vec![],
        }],
        missing_exec_targets: vec![CfgExecRecord {
            target: "missing.cfg".to_string(),
            source_file: "settings_ddnet.cfg".to_string(),
            line: 8,
            resolved_path: Some("C:/DDNet/missing.cfg".to_string()),
            missing: true,
        }],
    };

    let serialized = serde_json::to_value(analysis).expect("测试序列化应成功");

    assert_eq!(
        serialized["unbinds"][0]["source_file"],
        "settings_ddnet.cfg"
    );
    assert_eq!(
        serialized["execs"][0]["resolved_path"],
        "C:/DDNet/extra.cfg"
    );
    assert_eq!(serialized["execs"][0]["missing"], false);
    assert_eq!(serialized["conflicts"][0]["key"], "f1");
    assert_eq!(
        serialized["missing_exec_targets"][0]["target"],
        "missing.cfg"
    );
}
