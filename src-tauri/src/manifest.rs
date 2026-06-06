use crate::models::UpdateManifest;
use reqwest::Url;
use std::net::{IpAddr, Ipv6Addr};
use std::time::Duration;

const MAX_MANIFEST_BYTES: usize = 1_048_576;
const TRUSTED_MANIFEST_HOSTS: &[&str] = &["ddrace.cn", "raw.githubusercontent.com"];
const TRUSTED_ASSET_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "raw.githubusercontent.com",
    "ddrace.cn",
];

/// 解析更新 manifest JSON，并校验基础结构约束。
pub fn parse_manifest(input: &str) -> Result<UpdateManifest, String> {
    let manifest: UpdateManifest =
        serde_json::from_str(input).map_err(|error| format!("invalid manifest json: {error}"))?;

    if manifest.schema_version == 0 {
        return Err("manifest schema_version must be greater than 0".to_string());
    }

    if manifest.clients.is_empty() {
        return Err("manifest must contain at least one client".to_string());
    }

    validate_manifest_schema(&manifest)?;

    Ok(manifest)
}

/// 构造并校验 manifest URL，确保只访问公开 HTTPS 地址。
pub fn build_manifest_url(url: &str, proxy_base_url: Option<&str>) -> Result<Url, String> {
    let final_url = match proxy_base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), url),
        None => url.to_string(),
    };
    let parsed =
        Url::parse(&final_url).map_err(|error| format!("invalid manifest url: {error}"))?;

    validate_manifest_url(&parsed)?;

    Ok(parsed)
}

/// 从远程地址拉取更新 manifest，并复用本地解析校验逻辑。
pub async fn fetch_manifest(
    url: &str,
    proxy_base_url: Option<&str>,
) -> Result<UpdateManifest, String> {
    let final_url = build_manifest_url(url, proxy_base_url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("failed to fetch manifest: {error}"))?;

    let response = client
        .get(final_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch manifest: {error}"))?;
    let text = read_limited_manifest_response(response).await?;

    parse_manifest(&text)
}

fn validate_manifest_schema(manifest: &UpdateManifest) -> Result<(), String> {
    for client in &manifest.clients {
        if client.client_id.trim().is_empty() {
            return Err("manifest client_id must not be empty".to_string());
        }
        if client.channel.trim().is_empty() {
            return Err("manifest client channel must not be empty".to_string());
        }
        if client.version.trim().is_empty() {
            return Err("manifest client version must not be empty".to_string());
        }
        if client.release_notes.trim().is_empty() {
            return Err("manifest client release_notes must not be empty".to_string());
        }
        if client.assets.is_empty() {
            return Err("manifest client assets must not be empty".to_string());
        }

        for asset in &client.assets {
            if asset.platform.trim().is_empty() {
                return Err("manifest asset platform must not be empty".to_string());
            }
            if asset.asset_url.trim().is_empty() {
                return Err("manifest asset_url must not be empty".to_string());
            }
            if asset.sha256.trim().is_empty() {
                return Err("manifest asset sha256 must not be empty".to_string());
            }
            if !is_sha256_hex(&asset.sha256) {
                return Err("manifest asset sha256 must be 64 ASCII hex chars".to_string());
            }
            if asset.size == 0 {
                return Err("manifest asset size must be greater than 0".to_string());
            }

            let asset_url = Url::parse(&asset.asset_url)
                .map_err(|error| format!("invalid manifest asset_url: {error}"))?;
            validate_asset_url(&asset_url)?;
        }
    }

    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_manifest_url(url: &Url) -> Result<(), String> {
    let host = validate_public_https_url(url)?;
    validate_trusted_host(
        &host,
        TRUSTED_MANIFEST_HOSTS,
        "manifest url host is not trusted",
    )
}

fn validate_asset_url(url: &Url) -> Result<(), String> {
    let host = validate_public_https_url(url)?;
    validate_trusted_host(
        &host,
        TRUSTED_ASSET_HOSTS,
        "manifest asset_url host is not trusted",
    )
}

fn validate_public_https_url(url: &Url) -> Result<String, String> {
    if url.scheme() != "https" {
        return Err("manifest url must use https".to_string());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "manifest url must include host".to_string())?;
    let lower_host = host.trim_end_matches('.').to_ascii_lowercase();
    if lower_host == "localhost" || lower_host.ends_with(".localhost") {
        return Err("manifest url host must be public".to_string());
    }

    let ip_host = normalized_ip_host(host);
    if let Ok(ip) = ip_host.parse::<IpAddr>() {
        validate_public_ip(ip)?;
    }

    Ok(lower_host)
}

fn validate_trusted_host(host: &str, allowed_hosts: &[&str], error: &str) -> Result<(), String> {
    if allowed_hosts.contains(&host) {
        return Ok(());
    }

    Err(error.to_string())
}

fn validate_public_ip(ip: IpAddr) -> Result<(), String> {
    let is_blocked = match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return validate_public_ip(IpAddr::V4(ipv4));
            }

            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || is_ipv6_unique_local(&ipv6)
                || is_ipv6_unicast_link_local(&ipv6)
        }
    };

    if is_blocked {
        return Err("manifest url host must be public".to_string());
    }

    Ok(())
}

fn normalized_ip_host(host: &str) -> &str {
    host.trim_start_matches('[').trim_end_matches(']')
}

fn is_ipv6_unique_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_ipv6_unicast_link_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

async fn read_limited_manifest_response(mut response: reqwest::Response) -> Result<String, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MANIFEST_BYTES as u64)
    {
        return Err(format!(
            "manifest response exceeds {MAX_MANIFEST_BYTES} bytes"
        ));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("failed to read manifest response: {error}"))?
    {
        if bytes.len() + chunk.len() > MAX_MANIFEST_BYTES {
            return Err(format!(
                "manifest response exceeds {MAX_MANIFEST_BYTES} bytes"
            ));
        }
        bytes.extend_from_slice(&chunk);
    }

    String::from_utf8(bytes)
        .map_err(|error| format!("manifest response is not valid UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{build_manifest_url, parse_manifest};

    const VALID_SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn parses_manifest_with_qmclient_asset() {
        let manifest = parse_manifest(
            r#"{
                "schema_version": 1,
                "clients": [
                    {
                        "client_id": "qmclient",
                        "channel": "stable",
                        "version": "18.9.1",
                        "release_notes": "QmClient stable release",
                        "assets": [
                            {
                                "platform": "windows-x86_64",
                                "asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
                                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                                "size": 981467136
                            }
                        ]
                    }
                ]
            }"#,
        )
        .expect("测试 manifest 应解析成功");

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.clients[0].client_id, "qmclient");
        assert_eq!(manifest.clients[0].assets[0].platform, "windows-x86_64");
    }

    #[test]
    fn rejects_zero_schema_version() {
        let error = parse_manifest(
            r#"{
                "schema_version": 0,
                "clients": [
                    {
                        "client_id": "qmclient",
                        "channel": "stable",
                        "version": "18.9.1",
                        "release_notes": "QmClient stable release",
                        "assets": []
                    }
                ]
            }"#,
        )
        .expect_err("schema_version 为 0 应被拒绝");

        assert_eq!(error, "manifest schema_version must be greater than 0");
    }

    #[test]
    fn rejects_empty_clients() {
        let error = parse_manifest(
            r#"{
                "schema_version": 1,
                "clients": []
            }"#,
        )
        .expect_err("空 clients 应被拒绝");

        assert_eq!(error, "manifest must contain at least one client");
    }

    #[test]
    fn prefixes_invalid_json_errors() {
        let error = parse_manifest("{").expect_err("非法 JSON 应被拒绝");

        assert!(
            error.starts_with("invalid manifest json:"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn build_manifest_url_accepts_public_https_url() {
        let url = build_manifest_url("https://ddrace.cn/manifest.json", None)
            .expect("可信 HTTPS URL 应被接受");

        assert_eq!(url.as_str(), "https://ddrace.cn/manifest.json");
    }

    #[test]
    fn build_manifest_url_rejects_untrusted_host() {
        let error = build_manifest_url("https://example.com/manifest.json", None)
            .expect_err("非 allowlist host 应被拒绝");

        assert_eq!(error, "manifest url host is not trusted");
    }

    #[test]
    fn build_manifest_url_rejects_http_url() {
        let error = build_manifest_url("http://example.com/manifest.json", None)
            .expect_err("HTTP URL 应被拒绝");

        assert_eq!(error, "manifest url must use https");
    }

    #[test]
    fn build_manifest_url_rejects_localhost_and_private_hosts() {
        for url in [
            "https://localhost/manifest.json",
            "https://localhost./manifest.json",
            "https://api.localhost/manifest.json",
            "https://127.0.0.1/manifest.json",
            "https://10.0.0.1/manifest.json",
            "https://169.254.1.1/manifest.json",
            "https://0.0.0.0/manifest.json",
            "https://[::1]/manifest.json",
            "https://[fc00::1]/manifest.json",
            "https://[fe80::1]/manifest.json",
            "https://[::ffff:127.0.0.1]/manifest.json",
        ] {
            assert!(
                build_manifest_url(url, None).is_err(),
                "{url} should be rejected"
            );
        }
    }

    #[test]
    fn build_manifest_url_rejects_http_proxy_base_url() {
        let error = build_manifest_url("/manifest.json", Some("http://proxy.example.com"))
            .expect_err("HTTP 代理前缀应被拒绝");

        assert_eq!(error, "manifest url must use https");
    }

    #[test]
    fn build_manifest_url_rejects_untrusted_proxy_base_url() {
        let error = build_manifest_url("/manifest.json", Some("https://example.com"))
            .expect_err("非 allowlist 代理前缀应被拒绝");

        assert_eq!(error, "manifest url host is not trusted");
    }

    #[test]
    fn rejects_empty_assets() {
        let error = parse_manifest(
            r#"{
                "schema_version": 1,
                "clients": [
                    {
                        "client_id": "qmclient",
                        "channel": "stable",
                        "version": "18.9.1",
                        "release_notes": "QmClient stable release",
                        "assets": []
                    }
                ]
            }"#,
        )
        .expect_err("空 assets 应被拒绝");

        assert_eq!(error, "manifest client assets must not be empty");
    }

    #[test]
    fn rejects_invalid_sha256() {
        let error = parse_manifest(
            r#"{
                "schema_version": 1,
                "clients": [
                    {
                        "client_id": "qmclient",
                        "channel": "stable",
                        "version": "18.9.1",
                        "release_notes": "QmClient stable release",
                        "assets": [
                            {
                                "platform": "windows-x86_64",
                                "asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
                                "sha256": "not-a-sha256",
                                "size": 981467136
                            }
                        ]
                    }
                ]
            }"#,
        )
        .expect_err("非法 sha256 应被拒绝");

        assert_eq!(error, "manifest asset sha256 must be 64 ASCII hex chars");
    }

    #[test]
    fn rejects_http_asset_url() {
        let error = parse_manifest(
            r#"{
                "schema_version": 1,
                "clients": [
                    {
                        "client_id": "qmclient",
                        "channel": "stable",
                        "version": "18.9.1",
                        "release_notes": "QmClient stable release",
                        "assets": [
                            {
                                "platform": "windows-x86_64",
                                "asset_url": "http://example.com/qmclient.zip",
                                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                                "size": 981467136
                            }
                        ]
                    }
                ]
            }"#,
        )
        .expect_err("HTTP asset_url 应被拒绝");

        assert_eq!(error, "manifest url must use https");
    }

    #[test]
    fn accepts_trusted_asset_url() {
        let manifest = parse_manifest(&valid_manifest()).expect("可信 asset_url 应被接受");

        assert_eq!(
            manifest.clients[0].assets[0].asset_url,
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip"
        );
    }

    #[test]
    fn rejects_untrusted_asset_url() {
        let input = valid_manifest().replace(
            r#""asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip""#,
            r#""asset_url": "https://example.com/qmclient.zip""#,
        );
        let error = parse_manifest(&input).expect_err("非 allowlist asset_url 应被拒绝");

        assert_eq!(error, "manifest asset_url host is not trusted");
    }

    #[test]
    fn rejects_empty_client_id() {
        let input = valid_manifest().replace(r#""client_id": "qmclient""#, r#""client_id": """#);
        let error = parse_manifest(&input).expect_err("空 client_id 应被拒绝");

        assert_eq!(error, "manifest client_id must not be empty");
    }

    #[test]
    fn rejects_empty_channel() {
        let input = valid_manifest().replace(r#""channel": "stable""#, r#""channel": """#);
        let error = parse_manifest(&input).expect_err("空 channel 应被拒绝");

        assert_eq!(error, "manifest client channel must not be empty");
    }

    #[test]
    fn rejects_empty_version() {
        let input = valid_manifest().replace(r#""version": "18.9.1""#, r#""version": """#);
        let error = parse_manifest(&input).expect_err("空 version 应被拒绝");

        assert_eq!(error, "manifest client version must not be empty");
    }

    #[test]
    fn rejects_empty_release_notes() {
        let input = valid_manifest().replace(
            r#""release_notes": "QmClient stable release""#,
            r#""release_notes": """#,
        );
        let error = parse_manifest(&input).expect_err("空 release_notes 应被拒绝");

        assert_eq!(error, "manifest client release_notes must not be empty");
    }

    #[test]
    fn rejects_empty_platform() {
        let input =
            valid_manifest().replace(r#""platform": "windows-x86_64""#, r#""platform": """#);
        let error = parse_manifest(&input).expect_err("空 platform 应被拒绝");

        assert_eq!(error, "manifest asset platform must not be empty");
    }

    #[test]
    fn rejects_empty_asset_url() {
        let input = valid_manifest().replace(
            r#""asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip""#,
            r#""asset_url": """#,
        );
        let error = parse_manifest(&input).expect_err("空 asset_url 应被拒绝");

        assert_eq!(error, "manifest asset_url must not be empty");
    }

    #[test]
    fn rejects_zero_asset_size() {
        let input = valid_manifest().replace(r#""size": 981467136"#, r#""size": 0"#);
        let error = parse_manifest(&input).expect_err("size 为 0 应被拒绝");

        assert_eq!(error, "manifest asset size must be greater than 0");
    }

    fn valid_manifest() -> String {
        format!(
            r#"{{
                "schema_version": 1,
                "clients": [
                    {{
                        "client_id": "qmclient",
                        "channel": "stable",
                        "version": "18.9.1",
                        "release_notes": "QmClient stable release",
                        "assets": [
                            {{
                                "platform": "windows-x86_64",
                                "asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
                                "sha256": "{VALID_SHA256}",
                                "size": 981467136
                            }}
                        ]
                    }}
                ]
            }}"#
        )
    }
}
