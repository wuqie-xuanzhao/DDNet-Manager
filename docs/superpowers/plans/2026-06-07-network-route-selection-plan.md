# Network Route Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a backend route-selection unit that combines the backend A+B spec network constraints with the latest download-chain explore report's multi-route probing direction.

**Architecture:** Keep manifest and download URL validation in `manifest.rs` / `download.rs`. Add a focused `network_route.rs` module that represents route candidates, records probe outcomes, and selects the fastest successful explicit route without weakening HTTPS, public-host, or enabled-host requirements. Test snippets use reserved `.invalid` domains as placeholders only; the real manifest is project-maintained and must come from configuration, not Workshop.

**Tech Stack:** Rust 2021, Tauri v2 backend, existing `NetworkRouteConfig`, `tokio`, `reqwest`, `cargo test`.

---

### Task 1: Route Selection Core

**Files:**
- Create: `src-tauri/src/network_route.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing tests**

Add tests in `src-tauri/src/network_route.rs` for:

```rust
#[test]
fn select_fastest_successful_route_prefers_lowest_elapsed_ms() {
    let fast = NetworkRouteCandidate::named(
        "proxy-fast",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://fast.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["fast.invalid".to_string()],
        },
    );
    let slow = NetworkRouteCandidate::named(
        "proxy-slow",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://slow.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["slow.invalid".to_string()],
        },
    );

    let selected = select_fastest_successful_route(vec![
        NetworkRouteProbeOutcome::success(slow.clone(), 120),
        NetworkRouteProbeOutcome::failure(NetworkRouteCandidate::direct(), "timeout"),
        NetworkRouteProbeOutcome::success(fast.clone(), 40),
    ])
    .expect("应选中最快成功 route");

    assert_eq!(selected.name, "proxy-fast");
    assert_eq!(selected.route, fast.route);
}

#[test]
fn select_fastest_successful_route_reports_all_failures() {
    let error = select_fastest_successful_route(vec![
        NetworkRouteProbeOutcome::failure(NetworkRouteCandidate::direct(), "timeout"),
        NetworkRouteProbeOutcome::failure(
            NetworkRouteCandidate::named(
                "proxy-a",
                NetworkRouteConfig {
                    mode: NetworkRouteMode::ProxyPrefix,
                    proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
                    mirror_template: None,
                    enabled_hosts: vec!["proxy.invalid".to_string()],
                },
            ),
            "503",
        ),
    ])
    .expect_err("全部失败应返回聚合错误");

    assert!(error.contains("direct: timeout"));
    assert!(error.contains("proxy-a: 503"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
make rust-test
```

Expected: FAIL because `network_route` module and selection types do not exist yet.

- [ ] **Step 3: Implement the minimal core**

Create `NetworkRouteCandidate`, `NetworkRouteProbeOutcome`, `SelectedNetworkRoute`, and `select_fastest_successful_route`. Keep this core pure and deterministic so it can be tested without real network flakiness.

- [ ] **Step 4: Expose the module**

Add to `src-tauri/src/main.rs`:

```rust
/// 网络路由候选选择与探测结果聚合。
pub mod network_route;
```

- [ ] **Step 5: Run tests**

Run:

```bash
make rust-test
```

Expected: PASS.

### Task 2: Probe Hook

**Files:**
- Modify: `src-tauri/src/network_route.rs`
- Test: `src-tauri/src/network_route.rs`

- [ ] **Step 1: Write the failing tests**

Add a test proving candidate URL construction reuses existing manifest route validation:

```rust
#[test]
fn candidate_manifest_url_rejects_unenabled_route_host() {
    let candidate = NetworkRouteCandidate::named(
        "proxy",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["other.invalid".to_string()],
        },
    );

    let error = candidate
        .manifest_probe_url("https://raw.githubusercontent.com/example/ddnet-manager-manifest/main/manifest.json")
        .expect_err("未启用的 route host 应被拒绝");

    assert_eq!(error, "network route host is not enabled");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
make rust-test
```

Expected: FAIL because `manifest_probe_url` is not implemented.

- [ ] **Step 3: Implement URL construction hook**

Add `NetworkRouteCandidate::manifest_probe_url(&self, manifest_url: &str) -> Result<reqwest::Url, String>` and call `crate::manifest::build_manifest_url_with_route`.

- [ ] **Step 4: Run verification**

Run:

```bash
make rust-test
make check-lint
```

Expected: both commands exit 0; `cargo audit` may still report existing upstream WARN.
