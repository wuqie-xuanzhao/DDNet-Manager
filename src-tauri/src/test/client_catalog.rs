use super::{
    catalog_entries, catalog_entry_by_id, ddnet_steam_url, match_catalog_entry,
    normalize_client_id, third_party_entry,
};

#[test]
fn normalize_client_id_migrates_legacy_ddnet_vanilla() {
    assert_eq!(normalize_client_id("ddnet_vanilla"), "ddnet");
    assert_eq!(normalize_client_id("ddnet"), "ddnet");
    assert_eq!(normalize_client_id("qmclient"), "qmclient");
    assert_eq!(normalize_client_id("third_party"), "third_party");
}

#[test]
fn catalog_entry_by_id_finds_known_clients_and_migrates_legacy() {
    assert_eq!(
        catalog_entry_by_id("qmclient").map(|entry| entry.client_id),
        Some("qmclient")
    );
    assert_eq!(
        catalog_entry_by_id("ddnet").map(|entry| entry.client_id),
        Some("ddnet")
    );
    // 旧 ID 经 normalize 后应命中 ddnet entry
    assert_eq!(
        catalog_entry_by_id("ddnet_vanilla").map(|entry| entry.client_id),
        Some("ddnet")
    );
    assert!(catalog_entry_by_id("unknown-client").is_none());
}

#[test]
fn match_catalog_entry_matches_by_alias_in_path() {
    let qmclient = match_catalog_entry("D:/Games/QmClient").expect("应匹配 QmClient");
    assert_eq!(qmclient.client_id, "qmclient");

    let taterclient = match_catalog_entry("/home/user/tclient-build").expect("应匹配 TaterClient");
    assert_eq!(taterclient.client_id, "taterclient");

    let ddnet = match_catalog_entry("C:/Steam/steamapps/common/ddnet").expect("应匹配 DDNet");
    assert_eq!(ddnet.client_id, "ddnet");
}

#[test]
fn match_catalog_entry_returns_none_when_no_alias_matches() {
    assert!(match_catalog_entry("D:/Games/random-shooter").is_none());
}

#[test]
fn catalog_entries_contains_core_clients_and_fallback() {
    let ids: Vec<&str> = catalog_entries()
        .iter()
        .map(|entry| entry.client_id)
        .collect();
    assert!(ids.contains(&"qmclient"));
    assert!(ids.contains(&"ddnet"));
    assert!(ids.contains(&"third_party"));
}

#[test]
fn third_party_entry_is_fallback_slot() {
    assert_eq!(third_party_entry().client_id, "third_party");
}

#[test]
fn ddnet_steam_url_points_to_steam_store_app() {
    assert!(ddnet_steam_url().contains("412220"));
}
