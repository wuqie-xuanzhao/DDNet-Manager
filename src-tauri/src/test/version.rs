#[test]
fn is_update_needed_compares_numeric_versions() {
    assert!(crate::version::is_update_needed(Some("1.2.3"), "1.2.4"));
    assert!(!crate::version::is_update_needed(Some("2.0.0"), "1.9.9"));
    assert!(!crate::version::is_update_needed(Some("v1.2.3"), "1.2.3"));
    assert!(crate::version::is_update_needed(None, "1.2.3"));
}

#[test]
fn is_update_needed_handles_uppercase_v_prefix_and_unequal_length_core() {
    // 大写 V 前缀必须与小写 v 一致处理，与 github_release::normalize_release_version 对齐。
    assert!(!crate::version::is_update_needed(Some("V1.2.3"), "1.2.3"));
    assert!(!crate::version::is_update_needed(Some("V1.2.3"), "v1.2.3"));
    // 不等长 core：1.2 与 1.2.0 语义相等（缺位补 0），不应触发更新。
    assert!(!crate::version::is_update_needed(Some("1.2"), "1.2.0"));
    assert!(!crate::version::is_update_needed(Some("1.2.0"), "1.2"));
}

#[test]
fn is_update_needed_release_vs_prerelease() {
    // 正式版 > 预发布版：1.2.3 已是正式版，不需要降到 1.2.3-beta
    assert!(!crate::version::is_update_needed(
        Some("1.2.3"),
        "1.2.3-beta"
    ));
    // 预发布 < 正式版：1.2.3-beta 需要升级到 1.2.3 正式版
    assert!(crate::version::is_update_needed(
        Some("1.2.3-beta"),
        "1.2.3"
    ));
}

#[test]
fn is_update_needed_prerelease_numeric_identifiers() {
    // 核心 bug 修复：rc.2 vs rc.10 必须按数值比较（10 > 2），
    // 旧实现的字符串比较会误判 "rc.10" < "rc.2"（'1' < '2'）导致不更新。
    assert!(crate::version::is_update_needed(
        Some("1.2.3-rc.2"),
        "1.2.3-rc.10"
    ));
    assert!(!crate::version::is_update_needed(
        Some("1.2.3-rc.10"),
        "1.2.3-rc.2"
    ));
}

#[test]
fn is_update_needed_prerelease_alpha_identifiers() {
    // beta < rc：字母标识符按 ASCII 比较，'b' < 'r'
    assert!(crate::version::is_update_needed(
        Some("1.2.3-beta"),
        "1.2.3-rc.1"
    ));
    assert!(!crate::version::is_update_needed(
        Some("1.2.3-rc.1"),
        "1.2.3-beta"
    ));
}

#[test]
fn is_update_needed_smoke_suffix_self_compare() {
    // 9.9.9-smoke 是 smoke 测试用版本号，自比较不应触发更新
    assert!(!crate::version::is_update_needed(
        Some("9.9.9-smoke"),
        "9.9.9-smoke"
    ));
    // smoke 到正式版需要更新
    assert!(crate::version::is_update_needed(
        Some("9.9.9-smoke"),
        "9.9.9"
    ));
}

#[test]
fn is_update_needed_nightly_date_versions() {
    // nightly rolling 模式下版本号形如 nightly-2026-06-30，
    // tag_name="nightly" 无法解析为数字版本，走 fallback 字符串比较。
    // 不同日期需要更新
    assert!(crate::version::is_update_needed(
        Some("nightly-2026-06-30"),
        "nightly-2026-07-01"
    ));
    // 同日期不需要更新
    assert!(!crate::version::is_update_needed(
        Some("nightly-2026-06-30"),
        "nightly-2026-06-30"
    ));
}

#[test]
fn is_update_needed_prerelease_same_core_different_pre() {
    // 同 core 不同 pre：alpha < beta < rc
    assert!(crate::version::is_update_needed(
        Some("1.0.0-alpha"),
        "1.0.0-beta"
    ));
    assert!(crate::version::is_update_needed(
        Some("1.0.0-beta"),
        "1.0.0-rc.1"
    ));
    // 反向不需要更新
    assert!(!crate::version::is_update_needed(
        Some("1.0.0-rc.1"),
        "1.0.0-beta"
    ));
}

#[test]
fn is_update_needed_ignores_build_metadata() {
    // semver 构建元数据（+build）不参与比较，应被剥离。
    // 1.2.3+build.123 与 1.2.3 语义相等，不应触发更新。
    assert!(!crate::version::is_update_needed(
        Some("1.2.3+build.123"),
        "1.2.3"
    ));
    assert!(!crate::version::is_update_needed(
        Some("1.2.3"),
        "1.2.3+build.123"
    ));
    // 预发布版本带构建元数据：1.2.3-rc.1+build.123 与 1.2.3-rc.1 相等。
    assert!(!crate::version::is_update_needed(
        Some("1.2.3-rc.1+build.123"),
        "1.2.3-rc.1"
    ));
    // 构建元数据不影响 core 比较：1.2.3+build 仍需要更新到 1.2.4。
    assert!(crate::version::is_update_needed(
        Some("1.2.3+build.123"),
        "1.2.4"
    ));
}
