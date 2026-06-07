use super::{
    build_update_asset, normalize_release_version, parse_github_sha256_digest, GitHubReleaseAsset,
    GitHubReleaseCheck, ReleaseSelection,
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
fn missing_digest_returns_manual_release_action() {
    let result = build_update_asset(ReleaseSelection {
        platform: "windows-x86_64".to_string(),
        tag_name: "v2.62.4".to_string(),
        asset: GitHubReleaseAsset {
            name: "QmClient-windows.zip".to_string(),
            browser_download_url: "https://github.com/example/release.zip".to_string(),
            size: 42,
            digest: None,
        },
    })
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
