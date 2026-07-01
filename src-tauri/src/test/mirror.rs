use crate::mirror::{
    build_candidate_urls, extract_mirror_hosts, resolve_asset_url, resolve_prefixes,
    DEFAULT_MIRROR_PREFIXES,
};

#[test]
fn resolve_asset_url_with_prefix_concatenates() {
    let url = "https://github.com/owner/repo/releases/download/v1/asset.zip";
    assert_eq!(
        resolve_asset_url(url, Some("https://gh-proxy.com/")),
        "https://gh-proxy.com/https://github.com/owner/repo/releases/download/v1/asset.zip"
    );
}

#[test]
fn resolve_asset_url_trims_trailing_slash_from_prefix() {
    let url = "https://github.com/x/y.zip";
    // trim_end_matches 会删除末尾所有连续斜杠，避免拼出双斜杠
    assert_eq!(
        resolve_asset_url(url, Some("https://gh-proxy.com//")),
        "https://gh-proxy.com/https://github.com/x/y.zip"
    );
}

#[test]
fn resolve_asset_url_returns_original_when_prefix_none() {
    let url = "https://github.com/x/y.zip";
    assert_eq!(resolve_asset_url(url, None), url);
}

#[test]
fn resolve_asset_url_returns_original_when_prefix_empty() {
    let url = "https://github.com/x/y.zip";
    assert_eq!(resolve_asset_url(url, Some("")), url);
    assert_eq!(resolve_asset_url(url, Some("   ")), url);
}

#[test]
fn build_candidate_urls_puts_original_first() {
    let url = "https://github.com/x/y.zip";
    let prefixes = vec!["https://gh-proxy.com/".to_string()];
    let candidates = build_candidate_urls(url, &prefixes);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0], url);
    assert_eq!(
        candidates[1],
        "https://gh-proxy.com/https://github.com/x/y.zip"
    );
}

#[test]
fn build_candidate_urls_returns_only_original_when_no_prefixes() {
    let url = "https://github.com/x/y.zip";
    let candidates = build_candidate_urls(url, &[]);
    assert_eq!(candidates, vec![url]);
}

#[test]
fn build_candidate_urls_skips_blank_prefixes() {
    let url = "https://github.com/x/y.zip";
    let prefixes = vec![
        "".to_string(),
        "  ".to_string(),
        "https://gh-proxy.com/".to_string(),
    ];
    let candidates = build_candidate_urls(url, &prefixes);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0], url);
}

#[test]
fn build_candidate_urls_dedups_duplicate_prefixes() {
    let url = "https://github.com/x/y.zip";
    let prefixes = vec![
        "https://gh-proxy.com/".to_string(),
        "https://gh-proxy.com/".to_string(),
        "https://mirror.ghproxy.com/".to_string(),
    ];
    let candidates = build_candidate_urls(url, &prefixes);
    // 原始 + 2 个去重后的反代
    assert_eq!(candidates.len(), 3);
}

#[test]
fn default_mirror_prefixes_is_non_empty() {
    assert!(!DEFAULT_MIRROR_PREFIXES.is_empty());
    for prefix in DEFAULT_MIRROR_PREFIXES {
        assert!(prefix.starts_with("https://"));
    }
}

#[test]
fn resolve_prefixes_falls_back_to_default_when_empty() {
    let resolved = resolve_prefixes(&[]);
    assert_eq!(resolved.len(), DEFAULT_MIRROR_PREFIXES.len());
    for (got, expected) in resolved.iter().zip(DEFAULT_MIRROR_PREFIXES.iter()) {
        assert_eq!(got, expected);
    }
}

#[test]
fn resolve_prefixes_uses_configured_when_non_empty() {
    let configured = vec!["https://custom-proxy.example/".to_string()];
    let resolved = resolve_prefixes(&configured);
    assert_eq!(resolved, vec!["https://custom-proxy.example/"]);
}

#[test]
fn resolve_prefixes_filters_blank_entries() {
    let configured = vec![
        "".to_string(),
        "  ".to_string(),
        "https://x.example/".to_string(),
    ];
    let resolved = resolve_prefixes(&configured);
    assert_eq!(resolved, vec!["https://x.example/"]);
}

#[test]
fn resolve_prefixes_dedups() {
    let configured = vec![
        "https://x.example/".to_string(),
        "https://x.example/".to_string(),
    ];
    let resolved = resolve_prefixes(&configured);
    assert_eq!(resolved, vec!["https://x.example/"]);
}

// ===== review C1：extract_mirror_hosts 测试覆盖 =====

#[test]
fn extract_mirror_hosts_returns_lowercase_hosts() {
    let prefixes = vec![
        "https://GH-PROXY.com/".to_string(),
        "https://Mirror.Example.com/".to_string(),
    ];
    let hosts = extract_mirror_hosts(&prefixes);
    assert_eq!(hosts, vec!["gh-proxy.com", "mirror.example.com"]);
}

#[test]
fn extract_mirror_hosts_dedups_duplicate_hosts() {
    let prefixes = vec![
        "https://gh-proxy.com/".to_string(),
        "https://gh-proxy.com".to_string(),   // 末尾无斜杠
        "https://gh-proxy.com//".to_string(), // 双斜杠
    ];
    let hosts = extract_mirror_hosts(&prefixes);
    assert_eq!(hosts, vec!["gh-proxy.com"]);
}

#[test]
fn extract_mirror_hosts_skips_blank_and_invalid_entries() {
    let prefixes = vec![
        "".to_string(),
        "   ".to_string(),
        "not-a-url".to_string(), // Url::parse 失败，静默跳过
        "https://valid.example/".to_string(),
    ];
    let hosts = extract_mirror_hosts(&prefixes);
    assert_eq!(hosts, vec!["valid.example"]);
}

#[test]
fn extract_mirror_hosts_returns_empty_for_empty_input() {
    assert!(extract_mirror_hosts(&[]).is_empty());
}

#[test]
fn extract_mirror_hosts_handles_default_prefixes() {
    // 默认反代前缀都应能解析出 host
    let defaults: Vec<String> = DEFAULT_MIRROR_PREFIXES
        .iter()
        .map(|s| s.to_string())
        .collect();
    let hosts = extract_mirror_hosts(&defaults);
    assert!(!hosts.is_empty());
    // 每个默认前缀都应解析出非空 host
    for host in &hosts {
        assert!(!host.is_empty());
    }
}
