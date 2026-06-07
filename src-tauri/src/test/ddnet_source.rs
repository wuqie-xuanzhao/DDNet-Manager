use super::select_official_asset_from_text;

#[test]
fn selects_windows_asset_with_sha256_from_official_text() {
    let html = r#"
            <a href="/downloads/DDNet-19.8.2-win64.zip">Windows x86_64</a>
            <a href="/downloads/DDNet-19.8.2-linux_x86_64.tar.xz">Linux x86_64</a>
            <a href="/downloads/DDNet-19.8.2-macos.dmg">macOS</a>
        "#;
    let sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  DDNet-19.8.2-win64.zip\n";

    let asset = select_official_asset_from_text(html, sha, "windows-x86_64")
        .expect("官方源文本应解析成功")
        .expect("应匹配 Windows 资产");

    assert_eq!(asset.version, "19.8.2");
    assert_eq!(asset.platform, "windows-x86_64");
    assert_eq!(
        asset.asset_url,
        "https://ddnet.org/downloads/DDNet-19.8.2-win64.zip"
    );
    assert_eq!(
        asset.sha256,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    );
}

#[test]
fn selects_linux_tar_xz_asset_with_sha256_from_official_text() {
    let html = r#"<a href="DDNet-19.8.2-linux_x86_64.tar.xz">Linux</a>"#;
    let sha = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  DDNet-19.8.2-linux_x86_64.tar.xz\n";

    let asset = select_official_asset_from_text(html, sha, "linux-x86_64")
        .expect("官方源文本应解析成功")
        .expect("应匹配 Linux 资产");

    assert_eq!(asset.version, "19.8.2");
    assert_eq!(
        asset.asset_url,
        "https://ddnet.org/downloads/DDNet-19.8.2-linux_x86_64.tar.xz"
    );
}

#[test]
fn selects_newest_semantic_version_for_platform() {
    let html = r#"
            <a href="/downloads/DDNet-19.9.1-win64.zip">Windows old</a>
            <a href="/downloads/DDNet-19.10.0-win64.zip">Windows new</a>
        "#;
    let sha = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  DDNet-19.9.1-win64.zip
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  DDNet-19.10.0-win64.zip
";

    let asset = select_official_asset_from_text(html, sha, "windows-x86_64")
        .expect("官方源文本应解析成功")
        .expect("应匹配 Windows 资产");

    assert_eq!(asset.version, "19.10.0");
    assert_eq!(
        asset.asset_url,
        "https://ddnet.org/downloads/DDNet-19.10.0-win64.zip"
    );
}

#[test]
fn returns_error_when_official_asset_lacks_sha256() {
    let html = r#"<a href="/downloads/DDNet-19.8.2-macos.dmg">macOS</a>"#;

    let error =
        select_official_asset_from_text(html, "", "macos").expect_err("缺少 sha256 时不能自动安装");

    assert!(error.contains("missing sha256"));
}
