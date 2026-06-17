//! 下载 URL 校验与流式下载的测试。

use crate::download::{download_asset_to_file, validate_download_url, DownloadFileRequest};

#[test]
fn validate_download_url_allows_local_smoke_hosts_when_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        for url in [
            "http://localhost/file.zip",
            "https://127.0.0.1/file.zip",
            "http://10.0.0.1/file.zip",
            "https://169.254.1.1/file.zip",
            "http://[::1]/file.zip",
            "https://[fc00::1]/file.zip",
        ] {
            validate_download_url(url).expect("显式开启 local smoke 后应允许本地下载地址");
        }
    });
}

#[test]
fn validate_download_url_still_rejects_public_http_when_local_smoke_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        let error = validate_download_url("http://example.com/file.zip")
            .expect_err("local smoke 开关不应放通公网 HTTP 下载地址");

        assert_eq!(error.to_string(), "download url must use https");
    });
}

#[test]
fn validate_download_url_rejects_ambiguous_numeric_hosts() {
    for host in ["127.1", "2130706433", "0177.0.0.1"] {
        let url = format!("https://{host}/file.zip");

        let error = validate_download_url(&url).expect_err("歧义数字 host 应被拒绝");

        assert_eq!(
            error.to_string(),
            "download url host must be public",
            "{host}"
        );
    }
}

#[test]
fn validate_download_url_accepts_github_release_redirect_host() {
    validate_download_url(
        "https://release-assets.githubusercontent.com/github-production-release-asset/example.zip",
    )
    .expect("GitHub Release 资产重定向 host 应可用于直连下载");
}

#[tokio::test]
async fn download_asset_to_file_rejects_private_hosts_before_network() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let cache_path = temp_dir.path().join("download.zip");

    for url in [
        "https://localhost/file.zip",
        "https://127.0.0.1/file.zip",
        "https://10.0.0.1/file.zip",
        "https://169.254.1.1/file.zip",
        "https://[::1]/file.zip",
        "https://[fc00::1]/file.zip",
        "https://[::ffff:127.0.0.1]/file.zip",
    ] {
        let error = download_asset_to_file(
            DownloadFileRequest {
                asset_url: url,
                cache_path: &cache_path,
                expected_size: 1,
                route: None,
            },
            None,
            |_| true,
        )
        .await
        .expect_err("私网或本机下载地址应被拒绝");

        assert_eq!(error, "download url host must be public");
    }
}

#[tokio::test]
async fn download_asset_to_file_rejects_untrusted_public_host_before_network() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let cache_path = temp_dir.path().join("download.zip");

    let error = download_asset_to_file(
        DownloadFileRequest {
            asset_url: "https://example.com/file.zip",
            cache_path: &cache_path,
            expected_size: 1,
            route: None,
        },
        None,
        |_| true,
    )
    .await
    .expect_err("非可信 host 应被拒绝");

    assert_eq!(error, "download url host is not trusted");
}
