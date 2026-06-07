#[test]
fn is_update_needed_compares_numeric_versions() {
    assert!(crate::version::is_update_needed(Some("1.2.3"), "1.2.4"));
    assert!(!crate::version::is_update_needed(Some("2.0.0"), "1.9.9"));
    assert!(!crate::version::is_update_needed(Some("v1.2.3"), "1.2.3"));
    assert!(crate::version::is_update_needed(None, "1.2.3"));
}
