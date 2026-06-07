# DDNet Manager Next MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `docs/superpowers/specs/2026-06-07-ddnet-manager-next-mvp-spec.md` 收口为 0.1.0 MVP：客户端识别、注册、默认启动、无需普通用户手填 manifest 的更新检查、下载校验安装、资源/Binds 禁用和可验收状态展示。

**Architecture:** 后端以静态 `ClientCatalog` 作为客户端识别和更新源权威，`client_scan` 只负责把真实目录映射成带置信度、来源、兼容性状态的 `ClientInstallation`。更新检查新增 `update_source` 分派层，普通流程按 `client_id + channel` 解析 GitHub Release、DDNet 官网或 WebsiteSource，ManifestSource 仅保留为高级备用入口。前端继续使用集中 IPC 封装，主导航只发布启动、客户端、更新、设置闭环，资源/Binds 只显示不可用状态或隐藏。

**Tech Stack:** React 19 + TypeScript 6 + Vite 8 + Tailwind CSS v4；Tauri v2 + Rust 2021 + tokio + reqwest + serde + rusqlite；统一命令入口使用 `make`。

---

## File Structure

- Modify: `src-tauri/src/models.rs`
  - 增加 `ClientInstallSource`、`ClientConfidence`、`ClientCompatibility`、`CompatibilityReason`、`UpdateSourceKind`、`UpdateAction`、扩展 `ClientInstallation`、`ClientUpdateCheck`、`CheckClientUpdateRequest`、`StartUpdateDownloadRequest`。
- Create: `src-tauri/src/client_catalog.rs`
  - 内置 `qmclient`、`taterclient`、`bestclient`、`cactusclient`、`ddnet`、`third_party` 的平台、别名、可执行候选、更新源和官网/Steam/GitHub 管理入口。
- Modify: `src-tauri/src/client_scan.rs`
  - 使用 catalog 识别目录、迁移 `ddnet_vanilla` 到 `ddnet`、识别 Windows/macOS/Linux 候选、Steam 安装来源、兼容性和缺失项。
- Modify: `src-tauri/src/registry.rs`
  - 读取旧 JSON 时兼容新增字段默认值，并把旧 `ddnet_vanilla` 记录保存为 `ddnet`。
- Create: `src-tauri/src/update_source.rs`
  - 统一 `check_client_update` 和 `start_update_download` 的更新源分派；GitHubReleaseSource、WebsiteSource、ManifestSource 入口在此聚合。
- Create: `src-tauri/src/github_release.rs`
  - 查询 GitHub latest release，按 catalog 平台资产匹配，读取 asset digest 或拒绝缺少 sha256 的自动安装。
- Create: `src-tauri/src/ddnet_source.rs`
  - MVP 读取 DDNet 官网下载页和 `sha256sums.txt`，生成可校验资产；GitHub 只作为版本参考。
- Modify: `src-tauri/src/download.rs`
  - 按资产扩展名设置缓存文件后缀，保留 zip 安装，Linux tar.xz/macOS dmg 在没有平台实现时返回明确不可自动安装错误。
- Modify: `src-tauri/src/commands.rs`
  - `check_client_update` 不再要求普通流程传 manifest URL；`start_update_download` 复用同一个更新源分派；启动前复检健康状态。
- Modify: `src-tauri/src/main.rs`
  - 注册新增模块。
- Modify: `src/types.ts`
  - 同步后端新增类型和请求字段。
- Modify: `src/lib/tauri.ts`
  - 保持集中 IPC，新增 `launchDefaultClient` 或复用现有封装时补充类型。
- Modify: `src/App.tsx`
  - 导航只保留启动、客户端、更新；资源/Binds 不作为 0.1.0 可用入口。
- Modify: `src/components/launch/LaunchPanel.tsx`
  - 展示默认客户端、健康状态、兼容性状态、安装路径、版本和未配置引导。
- Modify: `src/components/clients/ClientManager.tsx`
  - 展示 install source、confidence、compatibility、管理入口，并阻止外部/Steam 安装被误认为可覆盖更新。
- Modify: `src/components/update/UpdatePanel.tsx`
  - 普通流程移除 manifest 输入，按默认客户端检查更新；高级 ManifestSource 折叠保留。
- Modify: `src/components/resources/ResourcePanel.tsx`
  - 只显示 `当前版本不可用，后续版本添加。`
- Modify: `src/components/binds/BindsPanel.tsx`
  - 只显示 `当前版本不可用，后续版本添加。`
- Test: `src-tauri/src/client_scan/tests.rs`
  - 增加 catalog 识别、ddnet 迁移、macOS bundle、Linux executable、Steam source、兼容性原因测试。
- Test: `src-tauri/src/manifest/tests.rs` and `src-tauri/src/commands/tests.rs`
  - 增加默认 manifest source 备用 URL、WebsiteSource 手动动作、缺 sha256 拒绝测试。
- Test: `src-tauri/src/download/tests.rs`
  - 增加非 zip 自动安装拒绝和缓存扩展名测试。

## Task 1: Extend IPC Models

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src/types.ts`
- Test: `src-tauri/src/models.rs`

- [ ] **Step 1: Write failing Rust serialization tests**

Add tests proving new fields serialize as snake_case and preserve old required fields:

```rust
#[test]
fn serializes_client_installation_with_mvp_metadata() {
    let installation = ClientInstallation {
        id: "ddnet-main".to_string(),
        client_id: "ddnet".to_string(),
        display_name: "DDNet".to_string(),
        install_dir: "D:/SteamLibrary/steamapps/common/DDNet".to_string(),
        executable_path: "D:/SteamLibrary/steamapps/common/DDNet/DDNet.exe".to_string(),
        storage_cfg_path: "D:/SteamLibrary/steamapps/common/DDNet/storage.cfg".to_string(),
        data_dir: "D:/SteamLibrary/steamapps/common/DDNet/data".to_string(),
        user_data_dir: None,
        version: Some("19.8.2".to_string()),
        is_default: true,
        health: ClientHealth::Ok,
        missing_items: Vec::new(),
        install_source: ClientInstallSource::Steam,
        confidence: ClientConfidence::Verified,
        manager_owned: false,
        compatibility: ClientCompatibility {
            status: CompatibilityStatus::Supported,
            can_launch: true,
            launch_verified: false,
            reasons: Vec::new(),
            last_launch_result: None,
        },
        upstream_url: Some("https://store.steampowered.com/app/412220/DDNet/".to_string()),
        last_scanned_at: Some("2026-06-07T12:00:00Z".to_string()),
    };

    let serialized = serde_json::to_value(installation).expect("测试序列化应成功");
    assert_eq!(serialized["client_id"], "ddnet");
    assert_eq!(serialized["install_source"], "steam");
    assert_eq!(serialized["confidence"], "verified");
    assert_eq!(serialized["compatibility"]["status"], "supported");
}
```

- [ ] **Step 2: Run the failing test**

Run: `make rust-test`

Expected: FAIL because the new enums and fields are not defined yet.

- [ ] **Step 3: Implement model additions**

Add serde enums with Chinese `///` docs:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientInstallSource { OfficialDownload, Steam, Manual, Manager }

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientConfidence { Verified, Compatible, Partial, Unsupported }

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus { Supported, Unsupported, Risky, Unknown, Verified }
```

Extend `ClientInstallation` with `#[serde(default)]` where old registry JSON must remain readable.

- [ ] **Step 4: Update TypeScript types**

Add exact TS unions mirroring Rust:

```typescript
export type ClientInstallSource = "official_download" | "steam" | "manual" | "manager";
export type ClientConfidence = "verified" | "compatible" | "partial" | "unsupported";
export type CompatibilityStatus = "supported" | "unsupported" | "risky" | "unknown" | "verified";
```

- [ ] **Step 5: Verify**

Run: `make rust-test`

Expected: PASS for model serialization tests.

## Task 2: Add ClientCatalog and Catalog-Based Scan

**Files:**
- Create: `src-tauri/src/client_catalog.rs`
- Modify: `src-tauri/src/client_scan.rs`
- Modify: `src-tauri/src/client_scan/tests.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write failing catalog scan tests**

Add tests:

```rust
#[test]
fn validate_client_dir_migrates_ddnet_vanilla_to_ddnet() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path().join("DDNet");
    std::fs::create_dir(&install_dir).expect("测试安装目录应创建成功");
    std::fs::write(install_dir.join("DDNet.exe"), b"MZ").expect("测试可执行文件应写入成功");
    std::fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    std::fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

    let installation = scan::validate_client_dir(&install_dir).expect("完整目录应验证成功");

    assert_eq!(installation.client_id, "ddnet");
    assert_ne!(installation.client_id, "ddnet_vanilla");
}
```

Also add macOS `.app/Contents/MacOS/DDNet` and Linux `DDNet` executable tests.

- [ ] **Step 2: Run the failing tests**

Run: `make rust-test`

Expected: FAIL because catalog module and platform candidates are not implemented.

- [ ] **Step 3: Implement `client_catalog.rs`**

Define static catalog entries with:

```rust
pub struct ClientCatalogEntry {
    pub client_id: &'static str,
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub executable_candidates: PlatformCandidates,
    pub required_markers: &'static [&'static str],
    pub update_source: UpdateSourceDescriptor,
    pub upstream_url: Option<&'static str>,
}
```

Include entries for `qmclient`, `taterclient`, `bestclient`, `cactusclient`, `ddnet`, and fallback `third_party`.

- [ ] **Step 4: Rework `client_scan` identity inference**

Replace path-only `infer_client_identity` with catalog matching:

- Normalize `ddnet_vanilla` as `ddnet`.
- Steam path sets `install_source = steam`, `manager_owned = false`.
- Known aliases plus executable/marker match set `confidence = verified`.
- DDNet-compatible unknown complete directory sets `client_id = third_party`, `confidence = compatible`.
- Missing executable/storage/data populates `missing_items`.

- [ ] **Step 5: Verify**

Run: `make rust-test`

Expected: PASS for scan tests.

## Task 3: Add UpdateSource Dispatch

**Files:**
- Create: `src-tauri/src/update_source.rs`
- Create: `src-tauri/src/github_release.rs`
- Modify: `src-tauri/src/manifest.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands/tests.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write failing update source tests**

Add tests proving ordinary requests can omit `manifest_url`:

```rust
#[test]
fn manifest_url_is_optional_for_catalog_update_sources() {
    let request = CheckClientUpdateRequest {
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        manifest_url: None,
        platform: Some("windows-x86_64".to_string()),
        network_route: None,
        use_manifest_source: false,
    };

    assert!(!crate::commands::request_requires_manifest_url(&request));
}
```

Add the inverse for advanced ManifestSource.

- [ ] **Step 2: Run failing tests**

Run: `make rust-test`

Expected: FAIL because `use_manifest_source` and dispatch helpers do not exist.

- [ ] **Step 3: Implement dispatch**

`update_source::check_client_update` behavior:

- If `request.use_manifest_source == true`, require `manifest_url`.
- `qmclient`, `taterclient`, `bestclient` use GitHubReleaseSource.
- `ddnet` uses DDNet source; if incomplete in current environment, return clear error instead of fake asset.
- `cactusclient` returns WebsiteSource manual action with no downloadable asset.
- `third_party` returns no update.

- [ ] **Step 4: Implement GitHubReleaseSource minimally**

Use `reqwest` to call `https://api.github.com/repos/{owner}/{repo}/releases/latest`, set a user agent, choose asset by catalog platform pattern, and reject missing sha256/digest with:

```text
update asset has no sha256; automatic install is disabled
```

- [ ] **Step 5: Wire commands**

Change `check_client_update` and `prepare_update_download_job` to call `update_source` instead of directly requiring manifest for ordinary flow.

- [ ] **Step 6: Verify**

Run: `make rust-test`

Expected: PASS for dispatch tests without network by testing helpers; network integration remains manual.

## Task 4: Download and Install Boundaries

**Files:**
- Modify: `src-tauri/src/download.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/download/tests.rs`

- [ ] **Step 1: Write failing tests**

Add tests for cache extension and unsupported archive install:

```rust
#[test]
fn create_download_job_preserves_tar_xz_extension() {
    let update = update_with_asset_url("https://github.com/example/client/releases/download/v1/client-linux.tar.xz");
    let job = crate::download::create_download_job("client-1", &update, Path::new("cache"));
    assert!(job.cache_path.ends_with(".tar.xz"));
}
```

- [ ] **Step 2: Run failing tests**

Run: `make rust-test`

Expected: FAIL because cache path currently always ends with `.zip`.

- [ ] **Step 3: Implement archive suffix and install guard**

Keep Windows zip automatic install. For `.tar.xz` and `.dmg`, return explicit product-safe errors until platform implementation is complete:

```text
linux tar.xz manager-owned install is not available on this platform build
macOS dmg manager-owned install is not available on this platform build
```

- [ ] **Step 4: Verify**

Run: `make rust-test`

Expected: PASS.

## Task 5: Frontend MVP Information Architecture

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/launch/LaunchPanel.tsx`
- Modify: `src/components/clients/ClientManager.tsx`
- Modify: `src/components/update/UpdatePanel.tsx`
- Modify: `src/components/resources/ResourcePanel.tsx`
- Modify: `src/components/binds/BindsPanel.tsx`
- Modify: `src/types.ts`

- [ ] **Step 1: Update TS types first**

Mirror backend fields exactly. Avoid `any`.

- [ ] **Step 2: Hide or disable non-MVP nav**

Remove `resources` and `binds` from `navItems`. Keep components as unavailable screens in case direct internal routing uses them.

- [ ] **Step 3: Update launch/client display**

Show separate health and compatibility labels. Use exact labels:

- `可启动`
- `可能无法启动`
- `当前系统不可用`
- `兼容性未知，建议手动测试启动`
- `已验证可启动`

- [ ] **Step 4: Update update panel**

Remove ordinary manifest URL input. Keep advanced ManifestSource collapsed and labeled:

```text
自维护 manifest / 调试用途
```

- [ ] **Step 5: Verify TypeScript**

Run: `make check`

Expected: PASS for TS build and cargo check.

## Task 6: Registry Compatibility and Startup Safety

**Files:**
- Modify: `src-tauri/src/registry.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/registry.rs`

- [ ] **Step 1: Write failing registry migration test**

Insert JSON with `client_id = "ddnet_vanilla"` and no new metadata fields, then assert list returns `client_id = "ddnet"`.

- [ ] **Step 2: Implement normalization**

After deserializing a client record, run a helper:

```rust
fn normalize_client_installation(client: &mut ClientInstallation) {
    if client.client_id == "ddnet_vanilla" {
        client.client_id = "ddnet".to_string();
    }
}
```

- [ ] **Step 3: Launch default health recheck**

Before launch, validate the saved install directory again and refuse launch if health is not `ok` or compatibility is `unsupported`.

- [ ] **Step 4: Verify**

Run: `make rust-test`

Expected: PASS.

## Task 7: Acceptance Gate and Review

**Files:**
- Potentially all modified files.

- [ ] **Step 1: Run local gate**

Run: `make check-lint`

Expected: PASS. Any FAIL must be fixed before completion.

- [ ] **Step 2: Run frontend build**

Run: `make build`

Expected: PASS.

- [ ] **Step 3: Desktop smoke**

Run: `make tauri-dev`

Expected: Desktop window opens, startup page loads, default client flow is usable on the current Windows machine.

- [ ] **Step 4: Dispatch read-only review subagent if available**

Because this touches IPC, Rust core logic, update/download behavior, and frontend contract, dispatch a read-only review subagent using `/chinese-code-review` and `/code-review-excellence`. The subagent must not modify files and must return final findings before the work is called reviewed.

- [ ] **Step 5: Final report**

Report:

- Commands run and results.
- Files changed.
- Which MVP acceptance items are complete.
- Which macOS/Linux installation items require platform-side manual verification or future implementation.

## Self-Review

- Spec coverage: The plan covers client catalog/scanning, `ddnet_vanilla` migration, update source dispatch, GitHub/Website/Manifest behavior, network route reuse, download safety, registry consistency, frontend IA, and resources/Binds disabling. Full macOS `.dmg` and Linux `.tar.xz` Manager-owned automatic install are planned as guarded boundaries unless implemented on those platforms.
- Placeholder scan: No task uses TBD/TODO/fill-in placeholders; each task names exact files and expected verification commands.
- Type consistency: Rust enum and TS union names match snake_case serialization. `ClientInstallation` metadata fields are introduced before scan, registry, and frontend tasks use them.
