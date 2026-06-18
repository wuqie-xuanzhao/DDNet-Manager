use super::{
    catalog_entries, catalog_entry_by_id, ddnet_steam_url, match_catalog_by_hash,
    match_catalog_by_pe, match_catalog_entry, normalize_client_id, third_party_entry,
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
fn catalog_entries_contains_core_clients_and_excludes_third_party() {
    // third_party 是 fallback slot，由 third_party_entry() 单独返回，不在 CATALOG
    // 里（不进 gallery 显示），见 client_catalog.rs:third_party_entry 的注释。
    let ids: Vec<&str> = catalog_entries()
        .iter()
        .map(|entry| entry.client_id)
        .collect();
    assert!(ids.contains(&"qmclient"));
    assert!(ids.contains(&"ddnet"));
    assert!(
        !ids.contains(&"third_party"),
        "third_party 不应出现在 catalog_entries（仅作为 fallback slot）"
    );
}

#[test]
fn third_party_entry_is_fallback_slot() {
    assert_eq!(third_party_entry().client_id, "third_party");
}

#[test]
fn ddnet_steam_url_points_to_steam_store_app() {
    assert!(ddnet_steam_url().contains("412220"));
}

// ===== PE 元信息匹配 =====

#[test]
fn match_catalog_by_pe_returns_none_when_all_ddnet_forks_share_pe() {
    // 真实场景：所有 DDNet 衍生客户端（QmClient/TaterClient/BestClient/Cactus）的 PE
    // 都继承上游硬编码值 "DDNet Team" / "DDNet"，加上 ddnet 原版共 5 个 entry 都匹配，
    // PE 元信息无法区分 → 必须返回 None，让识别走路径 + sha256。
    assert!(
        match_catalog_by_pe(Some("DDNet Team"), Some("DDNet")).is_none(),
        "5 个 entry 都匹配时应返回 None，避免把衍生客户端误识别为 ddnet"
    );
}

#[test]
fn match_catalog_by_pe_returns_none_when_no_field_matches() {
    assert!(match_catalog_by_pe(Some("Unknown Studio"), Some("Unknown Game")).is_none());
}

#[test]
fn match_catalog_by_pe_returns_none_when_both_none() {
    // 非 PE 文件 / PE 无 VS_VERSION_INFO → 字段都 None → None
    assert!(match_catalog_by_pe(None, None).is_none());
}

#[test]
fn catalog_pe_fields_use_real_ddnet_upstream_values() {
    // 数据完整性回归：所有 DDNet 衍生客户端 PE 字段必须用真实值 "DDNet Team"/"DDNet"，
    // 而不是按 repo owner 推断的占位值（历史 bug 来源）。
    let ddnet_family: &[&str] = &["qmclient", "taterclient", "bestclient", "cactusclient"];
    for client_id in ddnet_family {
        let entry = catalog_entry_by_id(client_id).expect("catalog entry 应存在");
        assert!(
            entry
                .pe_company_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case("DDNet Team")),
            "{client_id} 的 pe_company_names 必须包含真实值 'DDNet Team'"
        );
        assert!(
            entry
                .pe_product_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case("DDNet")),
            "{client_id} 的 pe_product_names 必须包含真实值 'DDNet'"
        );
    }
}

#[test]
fn match_catalog_by_hash_returns_none_for_unknown_hash() {
    // catalog known_hashes 初始为空（等用户填实际 release sha256），任何 hash 都返回 None。
    // 真实命中场景由 registry 指纹库（用户下载记录）覆盖。
    assert!(match_catalog_by_hash("deadbeef").is_none());
}

#[test]
fn match_catalog_by_hash_is_case_insensitive() {
    // 防御：未来填充 known_hashes 时大小写比较应统一
    assert!(match_catalog_by_hash("ABCDEF").is_none());
    assert!(match_catalog_by_hash("abcdef").is_none());
}
