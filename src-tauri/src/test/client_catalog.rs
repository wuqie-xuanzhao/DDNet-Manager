use super::{
    catalog_entries, catalog_entry_by_id, ddnet_steam_url, match_catalog_by_hash,
    match_catalog_by_pe, match_catalog_entry, normalize_client_id, third_party_entry,
    PeMatchStrength,
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

// ===== PE 元信息匹配 =====

#[test]
fn match_catalog_by_pe_strong_when_company_and_product_both_match() {
    // DDNet 官方：CompanyName="DDNet Team", ProductName="DDNet"
    let (entry, strength) = match_catalog_by_pe(Some("DDNet Team"), Some("DDNet"))
        .expect("Company+Product 都匹配应命中 ddnet");
    assert_eq!(entry.client_id, "ddnet");
    assert_eq!(strength, PeMatchStrength::Strong);
}

#[test]
fn match_catalog_by_pe_weak_when_only_one_field_matches() {
    // 只匹配 CompanyName，未给 ProductName → Weak
    let (entry, strength) =
        match_catalog_by_pe(Some("wxj881027"), None).expect("Company 匹配应命中 qmclient");
    assert_eq!(entry.client_id, "qmclient");
    assert_eq!(strength, PeMatchStrength::Weak);
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
fn match_catalog_by_pe_is_case_insensitive() {
    let (entry, _) = match_catalog_by_pe(Some("QMCLIENT"), Some("QMCLIENT"))
        .expect("大小写不敏感匹配应命中 qmclient");
    assert_eq!(entry.client_id, "qmclient");
}

#[test]
fn match_catalog_by_pe_resolves_bestclient_vs_taterclient_distinction() {
    // 核心回归：BestClient PE 元信息（"BestProjectTeam" / "BestClient"）不应误判为 taterclient。
    // 路径匹配会因为 "tclient" 子串误判（taterclient aliases 含 "tclient"），
    // PE 元信息匹配直接按 CompanyName/Product 区分。
    let (entry, strength) = match_catalog_by_pe(Some("BestProjectTeam"), Some("BestClient"))
        .expect("PE 应精准识别 BestClient");
    assert_eq!(entry.client_id, "bestclient");
    assert_eq!(strength, PeMatchStrength::Strong);

    let (entry, _) = match_catalog_by_pe(Some("TaterClient"), Some("TaterClient"))
        .expect("PE 应精准识别 TaterClient");
    assert_eq!(entry.client_id, "taterclient");
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
