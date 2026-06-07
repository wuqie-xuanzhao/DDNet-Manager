use crate::models::{
    ClientCompatibility, ClientConfidence, ClientHealth, ClientInstallSource, ClientInstallation,
};

fn test_client(id: &str, is_default: bool) -> ClientInstallation {
    ClientInstallation {
        id: id.to_string(),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir: format!("C:/Games/{id}"),
        executable_path: format!("C:/Games/{id}/DDNet.exe"),
        storage_cfg_path: format!("C:/Games/{id}/storage.cfg"),
        data_dir: format!("C:/Games/{id}/data"),
        user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
        version: None,
        is_default,
        health: ClientHealth::Ok,
        missing_items: Vec::new(),
        install_source: ClientInstallSource::Manual,
        confidence: ClientConfidence::Compatible,
        manager_owned: false,
        compatibility: ClientCompatibility {
            can_launch: true,
            ..ClientCompatibility::default()
        },
        upstream_url: None,
        last_scanned_at: Some("2026-06-06T12:00:00Z".to_string()),
    }
}

#[test]
fn registry_upserts_lists_and_sets_single_default_client() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

    registry
        .upsert_client_installation(&test_client("qmclient-main", true))
        .expect("第一条客户端应保存成功");
    registry
        .upsert_client_installation(&test_client("ddnet-vanilla", false))
        .expect("第二条客户端应保存成功");
    registry
        .set_default_client("ddnet-vanilla")
        .expect("默认客户端应设置成功");

    let clients = registry
        .list_client_installations()
        .expect("客户端列表应读取成功");
    let default_client = registry
        .get_default_client()
        .expect("默认客户端查询应成功")
        .expect("应存在默认客户端");

    assert_eq!(clients.len(), 2);
    assert_eq!(default_client.id, "ddnet-vanilla");
    assert_eq!(clients.iter().filter(|client| client.is_default).count(), 1);
}

#[test]
fn registry_remove_does_not_delete_files_and_clears_default() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

    registry
        .upsert_client_installation(&test_client("qmclient-main", true))
        .expect("客户端应保存成功");
    registry
        .remove_client_installation("qmclient-main")
        .expect("客户端记录应移除成功");

    assert!(registry
        .get_default_client()
        .expect("默认客户端查询应成功")
        .is_none());
    assert!(registry
        .list_client_installations()
        .expect("客户端列表应读取成功")
        .is_empty());
}

#[test]
fn registry_normalizes_legacy_ddnet_vanilla_records() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let legacy_json = serde_json::json!({
        "id": "ddnet-legacy",
        "client_id": "ddnet_vanilla",
        "display_name": "DDNet",
        "install_dir": "D:/Games/DDNet",
        "executable_path": "D:/Games/DDNet/DDNet.exe",
        "storage_cfg_path": "D:/Games/DDNet/storage.cfg",
        "data_dir": "D:/Games/DDNet/data",
        "user_data_dir": null,
        "version": null,
        "is_default": true,
        "health": "ok",
        "last_scanned_at": "2026-06-06T12:00:00Z"
    })
    .to_string();

    registry
        .conn
        .execute(
            "INSERT INTO client_installations (
                    id, client_id, display_name, install_dir, executable_path,
                    storage_cfg_path, data_dir, user_data_dir, version, health,
                    last_scanned_at, is_default, client_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8, ?9, 1, ?10)",
            rusqlite::params![
                "ddnet-legacy",
                "ddnet_vanilla",
                "DDNet",
                "D:/Games/DDNet",
                "D:/Games/DDNet/DDNet.exe",
                "D:/Games/DDNet/storage.cfg",
                "D:/Games/DDNet/data",
                "\"ok\"",
                "2026-06-06T12:00:00Z",
                legacy_json
            ],
        )
        .expect("旧记录应写入成功");

    let clients = registry
        .list_client_installations()
        .expect("旧记录应读取成功");

    assert_eq!(clients[0].client_id, "ddnet");
}
