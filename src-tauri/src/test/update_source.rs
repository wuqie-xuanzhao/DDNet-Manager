use super::current_platform;

#[test]
fn linux_current_platform_includes_architecture() {
    let platform = current_platform_for("linux", "x86_64");

    assert_eq!(platform, "linux-x86_64");
}

#[test]
fn linux_arm64_current_platform_includes_architecture() {
    let platform = current_platform_for("linux", "aarch64");

    assert_eq!(platform, "linux-aarch64");
}

fn current_platform_for(os: &str, arch: &str) -> String {
    super::platform_from_os_arch(os, arch)
}

#[test]
fn current_platform_uses_runtime_constants_without_empty_result() {
    assert!(!current_platform().is_empty());
}
