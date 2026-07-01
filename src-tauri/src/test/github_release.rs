use std::collections::HashMap;

use super::{
    build_update_asset, nightly_version_from_published_at, normalize_release_version,
    parse_expanded_assets_digests, parse_github_sha256_digest, select_release_asset,
    GitHubReleaseAsset, GitHubReleaseCheck, GitHubReleaseResponse, ReleaseAssetSelection,
    ReleaseSelection,
};

#[test]
fn parses_github_sha256_digest() {
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    assert_eq!(
        parse_github_sha256_digest(&format!("sha256:{digest}")),
        Some(digest.to_string())
    );
    assert_eq!(parse_github_sha256_digest(digest), Some(digest.to_string()));
    assert!(parse_github_sha256_digest("sha256:not-valid").is_none());
}

#[test]
fn normalizes_release_version_prefix() {
    assert_eq!(normalize_release_version("v2.62.4"), "2.62.4");
    assert_eq!(normalize_release_version("V10.8.7"), "10.8.7");
    assert_eq!(normalize_release_version("19.8.2"), "19.8.2");
}

#[test]
fn parse_expanded_assets_digests_maps_asset_name_to_sha256() {
    // GitHub expanded_assets 端点返回的 HTML 片段结构（脱敏简化）。
    // digest 只出现在 clipboard-copy 元素的 aria-label + value 属性对里。
    let html = r#"
        <li>
            <a href="/o/r/releases/download/v1/QmClient-windows.zip"><span class="text-bold">QmClient-windows.zip</span></a>
            <clipboard-copy aria-label="Copy to clipboard digest for QmClient-windows.zip" value="sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"></clipboard-copy>
        </li>
        <li>
            <a href="/o/r/releases/download/v1/QmClient-android.apk"><span class="text-bold">QmClient-android.apk</span></a>
            <clipboard-copy aria-label="Copy to clipboard digest for QmClient-android.apk" value="sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"></clipboard-copy>
        </li>
    "#;
    let map = parse_expanded_assets_digests(html);
    assert_eq!(
        map.get("QmClient-windows.zip").map(String::as_str),
        Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    );
    assert_eq!(
        map.get("QmClient-android.apk").map(String::as_str),
        Some("fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210")
    );
}

#[test]
fn parse_expanded_assets_digests_skips_assets_without_digest() {
    let html = r#"
        <li>
            <a href="/o/r/releases/download/v1/no-digest.zip"><span class="text-bold">no-digest.zip</span></a>
        </li>
        <clipboard-copy aria-label="Copy to clipboard digest for with-digest.zip" value="sha256:1111111111111111111111111111111111111111111111111111111111111111"></clipboard-copy>
    "#;
    let map = parse_expanded_assets_digests(html);
    assert!(!map.contains_key("no-digest.zip"));
    assert_eq!(
        map.get("with-digest.zip").map(String::as_str),
        Some("1111111111111111111111111111111111111111111111111111111111111111")
    );
}

#[test]
fn build_update_asset_prefers_expanded_assets_digest() {
    let mut digests = HashMap::new();
    digests.insert(
        "QmClient-windows.zip".to_string(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
    );
    let result = build_update_asset(
        ReleaseSelection {
            platform: "windows-x86_64".to_string(),
            version: "2.62.4".to_string(),
            asset: GitHubReleaseAsset {
                name: "QmClient-windows.zip".to_string(),
                browser_download_url: "https://github.com/example/release.zip".to_string(),
                size: 42,
                digest: None,
            },
        },
        &digests,
    )
    .expect("应返回更新检查结果")
    .expect("应存在匹配资产");

    match result {
        GitHubReleaseCheck::Download { version, asset } => {
            assert_eq!(version, "2.62.4");
            assert_eq!(
                asset.sha256,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            );
        }
        GitHubReleaseCheck::Manual { .. } => panic!("expanded digest 存在时应返回下载动作"),
    }
}

#[test]
fn missing_digest_returns_manual_release_action() {
    let result = build_update_asset(
        ReleaseSelection {
            platform: "windows-x86_64".to_string(),
            version: "2.62.4".to_string(),
            asset: GitHubReleaseAsset {
                name: "QmClient-windows.zip".to_string(),
                browser_download_url: "https://github.com/example/release.zip".to_string(),
                size: 42,
                digest: None,
            },
        },
        &HashMap::new(),
    )
    .expect("缺少 digest 应返回手动动作")
    .expect("应存在匹配资产");

    match result {
        GitHubReleaseCheck::Manual { version, message } => {
            assert_eq!(version, "2.62.4");
            assert!(message.contains("sha256"));
        }
        GitHubReleaseCheck::Download { .. } => panic!("缺少 digest 不应返回下载动作"),
    }
}

#[test]
fn select_release_asset_prefers_expanded_digest_over_missing_api_digest() {
    use crate::client_catalog::catalog_entry_by_id;

    // 模拟真实场景：标准 release API 不返回 asset.digest，digest 来自 expanded_assets。
    let entry = catalog_entry_by_id("qmclient").expect("qmclient catalog entry 应存在");
    let release = GitHubReleaseResponse {
        tag_name: "v2.62.4".to_string(),
        html_url: "https://github.com/wxj881027/QmClient/releases/tag/v2.62.4".to_string(),
        body: None,
        assets: vec![GitHubReleaseAsset {
            name: "QmClient-windows.zip".to_string(),
            browser_download_url:
                "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                    .to_string(),
            size: 89531134,
            digest: None,
        }],
        prerelease: false,
        published_at: String::new(),
    };
    let mut digests = HashMap::new();
    digests.insert(
        "QmClient-windows.zip".to_string(),
        "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
    );

    let result = select_release_asset(ReleaseAssetSelection {
        entry,
        platform: "windows-x86_64",
        release,
        version: "2.62.4".to_string(),
        digests: &digests,
    })
    .expect("应返回更新检查结果")
    .expect("应存在匹配资产");

    match result {
        GitHubReleaseCheck::Download { version, asset } => {
            assert_eq!(version, "2.62.4");
            assert_eq!(asset.platform, "windows-x86_64");
            assert_eq!(
                asset.sha256,
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
            );
            assert_eq!(asset.size, 89531134);
        }
        GitHubReleaseCheck::Manual { .. } => {
            panic!("expanded digest 存在时应返回下载动作，而非降级手动")
        }
    }
}

#[test]
fn nightly_version_from_published_at_extracts_date_prefix() {
    // GitHub API 的 published_at 是 ISO 8601 UTC：2026-06-30T18:46:26Z
    // nightly 版本号取日期前缀，形成 nightly-2026-06-30，供 is_update_needed fallback 比较
    assert_eq!(
        nightly_version_from_published_at("2026-06-30T18:46:26Z"),
        "nightly-2026-06-30"
    );
    assert_eq!(
        nightly_version_from_published_at("2026-07-01T00:00:00Z"),
        "nightly-2026-07-01"
    );
}

#[test]
fn nightly_version_from_published_at_handles_short_input() {
    // 异常兜底：published_at 过短无法取到 10 位日期时返回 nightly-unknown，避免 panic
    assert_eq!(nightly_version_from_published_at(""), "nightly-unknown");
    assert_eq!(nightly_version_from_published_at("2026"), "nightly-unknown");
}

#[test]
fn nightly_version_from_published_at_rejects_non_date_format() {
    // 长度够 10 但不符合 YYYY-MM-DD 结构（位置 4/7 非 '-' 或其余位非数字）→ nightly-unknown
    assert_eq!(
        nightly_version_from_published_at("garbage-2026-06-30"),
        "nightly-unknown"
    );
    assert_eq!(
        nightly_version_from_published_at("20260630ABC"),
        "nightly-unknown"
    );
    // 合法日期格式但语义非法（月份 13、日 45）仍会通过——只做格式校验不校验日历合法性，
    // nightly 版本号只需可比较，无需语义合法的日期。
    assert_eq!(
        nightly_version_from_published_at("2026-13-45T00:00:00Z"),
        "nightly-2026-13-45"
    );
}
