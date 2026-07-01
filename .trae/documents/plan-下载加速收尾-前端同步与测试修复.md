# 实现计划：下载加速与多源竞速 —— 收尾（前端同步 + 测试修复）

基于 `docs/superpowers/specs/2026-06-24-下载加速与多源竞速方案.md` 与已批准的 `.trae/documents/plan-下载加速与多源竞速-P0.md`。前序会话已完成绝大部分后端实现，本计划只覆盖**剩余收尾工作**：后端测试修复、前端类型/UI 同步、前端测试 mock 同步、门禁验收。

## 现状分析

### 后端（已完成，无需再改实现）

经核对，以下后端文件均已按 P0 计划落地：

- [mirror.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/mirror.rs)：`resolve_asset_url` / `build_candidate_urls` / `resolve_prefixes` / `DEFAULT_MIRROR_PREFIXES`。拼接 bug 已修复（`format!("{}/{}", trimmed.trim_end_matches('/'), original)`）。
- [download/race.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/race.rs)：`select_best_source` 两段式竞速，`RaceWinner { url, head_start }` 已 `#[derive(Debug)]`，`response` 已声明 `mut`，`cancel` 用 `Option<&CancellationToken>`。
- [download/net.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs)：`validate_download_url(url, extra_hosts)`、`is_host_trusted`、`send_download_request` 支持 `start_offset` 续传、`download_asset_to_file` 接入竞速。
- [models.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs)：`NetworkRouteMode::AutoDetect`、`AppSettings.extra_trusted_hosts` + `mirror_prefixes`。
- [network_route.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/network_route.rs)：`resolve_effective_proxy` / `detect_system_proxy` / `detect_env_proxy` / `normalize_proxy_url` / `parse_windows_proxy_server` / `detect_windows_registry_proxy`（`#[cfg(windows)]`）。
- [commands/download.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/commands/download.rs) + [commands.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/commands.rs)：`DownloadTaskContext` 含 `mirror_prefixes` + `extra_hosts`，`run_download_loop` 用 `build_candidate_urls`。
- [Cargo.toml](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/Cargo.toml)：`winreg = "0.52"`（Windows target）+ `mockito = "1"`（dev）。

### 后端测试（存在失败，需修复）

- [test/mirror.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/mirror.rs)：11 个测试，拼接 bug 修复后应通过，待 `cargo test` 验证。
- [test/download/race.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/download/race.rs)：9 个 mockito 测试，`response` mut + `RaceWinner` Debug 修复后应通过，待验证。
- [test/network_route.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/network_route.rs)：**5 个 env 测试并发干扰**。文件顶部已声明 `static ENV_MUTEX: Mutex<()> = Mutex::new(())`，但 5 个操作 `HTTPS_PROXY`/`HTTP_PROXY` 的测试函数体**尚未加 `let _guard = ENV_MUTEX.lock().unwrap();`**，导致并发跑时互相覆盖 env 变量断言失败（如 `detect_env_proxy_reads_https_proxy_first` 拿到 `None`）。

### 前端（未开始，TS 编译会失败）

后端 `AppSettings` 已有 `extra_trusted_hosts` / `mirror_prefixes`，前端 [types.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/types.ts) 尚未同步，一旦后端返回带新字段的 JSON，前端类型不匹配；且 `NetworkRouteMode` 缺 `"auto_detect"` 字面量，`auto_detect` 模式下 [updateLogic.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.ts) 的 switch 会漏 case。

具体缺口：

- [types.ts:140](file:///e:/Coding/DDNet/DDNet-Manager/src/types.ts#L140)：`NetworkRouteMode = "direct" | "local_proxy"`，缺 `"auto_detect"`。
- [types.ts:147-158](file:///e:/Coding/DDNet/DDNet-Manager/src/types.ts#L147-L158)：`AppSettings` 缺 `extra_trusted_hosts` / `mirror_prefixes`。
- [settings.ts:3-14](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/settings.ts#L3-L14)：`defaultAppSettings` 缺新字段；`updateNetworkRoute` 只处理 `direct` vs 非 `direct`，未区分 `auto_detect`（不应要求 URL）。
- [updateLogic.ts:28-42](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.ts#L28-L42)：`buildNetworkRoute` 对非 direct 模式一律要求 URL，`auto_detect` 不需要；[updateLogic.ts:44-71](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.ts#L44-L71) `networkRouteLabel` / `networkRoutePlaceholder` / `networkRouteHint` 三个 switch 缺 `auto_detect` case（TS 会报 switch 不穷尽）。
- [UpdatePanel.tsx:807](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.tsx#L807)：mode 列表 `["direct", "local_proxy"]` 缺 `auto_detect`；[UpdatePanel.tsx:832](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.tsx#L832) 输入框显隐条件 `activeRouteMode !== "direct"` 应改为 `=== "local_proxy"`（`auto_detect` 不需要输入框）。
- 3 个前端测试构造 `AppSettings` 字面量，加必填字段后会 TS 报错：[UpdatePanel.test.tsx:105-116](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.test.tsx#L105-L116) `mockSettings`、[useClientInstaller.test.tsx:42-53](file:///e:/Coding/DDNet/DDNet-Manager/src/hooks/useClientInstaller.test.tsx#L42-L53) `baseSettings`、[useAppUpdater.test.tsx:26-37](file:///e:/Coding/DDNet/DDNet-Manager/src/hooks/useAppUpdater.test.tsx#L26-L37) `baseSettings`。

## 改动清单

### A. 后端测试修复

#### A1. [test/network_route.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/network_route.rs) —— 串行化 env 测试

给以下 5 个函数体开头加 `let _guard = ENV_MUTEX.lock().expect("env 测试串行锁应可获取");`：

- `detect_env_proxy_reads_https_proxy_first`（约 142 行）
- `detect_env_proxy_normalizes_missing_scheme`（约 154 行）
- `detect_env_proxy_returns_none_when_all_empty`（约 165 行）
- `builds_client_with_auto_detect_route_picks_up_env_proxy`（约 174 行）
- `auto_detect_route_without_env_proxy_builds_plain_client`（约 189 行）

**为什么**：`HTTPS_PROXY`/`HTTP_PROXY` 是进程级共享 env，Rust 默认并发跑 `#[test]`，并发设置/删除同一 env 变量会互相覆盖，导致断言拿到 `None` 或脏值。静态 `Mutex` 串行化所有动 env 的测试，`_guard` 在函数结束时自动释放。

**为什么用 `lock().expect` 而非 `lock().unwrap()`**：门禁脚本扫描非测试代码的 `unwrap`/`expect` 告警，但这是测试代码（`#[test]` 函数体内），允许；保持与已有测试风格一致用 `expect` 带中文说明。

#### A2. 验证 mirror / race 测试

A1 改完后跑 `cargo test --manifest-path src-tauri/Cargo.toml`。若 mirror / race 仍有失败，按报错就地修复（预期 A1 修完后全部通过，因为前序会话已修拼接 bug、`response` mut、`RaceWinner` Debug、`cancel.as_ref`）。若 race 测试因 mockito 并发起 server 端口冲突失败，给 race 测试模块也加一个 `static RACE_MUTEX` 串行化（兜底方案，仅在确认并发冲突时启用）。

### B. 前端类型同步

#### B1. [src/types.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/types.ts)

- L140：`export type NetworkRouteMode = "direct" | "local_proxy";` → `export type NetworkRouteMode = "direct" | "auto_detect" | "local_proxy";`
- L147-158 `AppSettings` 末尾（`allow_silent_update: boolean;` 后）追加：
  ```typescript
  /** 用户显式信任的额外下载 host（公共反代域名），对应后端 SSRF 白名单动态放行。 */
  extra_trusted_hosts: string[];
  /** 反代前缀列表；空时后端用 DEFAULT_MIRROR_PREFIXES 兜底。 */
  mirror_prefixes: string[];
  ```

### C. 前端逻辑同步

#### C1. [src/lib/settings.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/settings.ts)

- `defaultAppSettings`（L3-14）末尾追加 `extra_trusted_hosts: []` 和 `mirror_prefixes: []`。
- `updateNetworkRoute`（L20-32）在 `direct` 分支后加 `auto_detect` 分支：
  ```typescript
  if (mode === "direct") {
    return { ...settings, network_route: null };
  }
  if (mode === "auto_detect") {
    return { ...settings, network_route: { mode: "auto_detect", local_proxy_url: null } };
  }
  // local_proxy：保留现有 trimmedUrl 逻辑
  ```
  **为什么**：`auto_detect` 模式由后端 `detect_system_proxy` 解析代理，不需要用户填 URL，`local_proxy_url` 置 `null`。

#### C2. [src/lib/updateLogic.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.ts)

- `buildNetworkRoute`（L28-42）在 `direct` 分支后加 `auto_detect` 分支：
  ```typescript
  if (routeMode === "direct") {
    return null;
  }
  if (routeMode === "auto_detect") {
    return { mode: "auto_detect", local_proxy_url: null };
  }
  // local_proxy：保留现有 trimmedUrl + throw 逻辑
  ```
- `networkRouteLabel`（L44-51）加 `case "auto_detect": return "自动检测";`
- `networkRoutePlaceholder`（L54-61）加 `case "auto_detect": return "";`
- `networkRouteHint`（L64-71）加：
  ```typescript
  case "auto_detect":
    return "自动检测系统代理（环境变量 HTTPS_PROXY 或 Windows 系统代理）。适合已配置 Clash/V2Ray 系统代理的用户，无需手动填写地址。";
  ```

### D. 前端 UI

#### D1. [src/components/update/UpdatePanel.tsx](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.tsx)

- L807：`{(["direct", "local_proxy"] as const).map((mode) => {` → `{(["direct", "auto_detect", "local_proxy"] as const).map((mode) => {`
  **顺序理由**：`direct`（直连）→ `auto_detect`（自动检测，最常用）→ `local_proxy`（手动填写，最复杂）。`auto_detect` 放中间作为推荐默认选项的视觉位置。
- L832：`{activeRouteMode !== "direct" ? (` → `{activeRouteMode === "local_proxy" ? (`
  **为什么**：只有 `local_proxy` 需要用户填地址；`direct` 和 `auto_detect` 都不需要输入框。原条件 `!== "direct"` 在加了 `auto_detect` 后会错误地对 `auto_detect` 也显示输入框。

### E. 前端测试同步

#### E1. [src/components/update/UpdatePanel.test.tsx](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.test.tsx)

`mockSettings`（L105-116）末尾 `allow_silent_update: true` 后追加：
```typescript
  extra_trusted_hosts: [],
  mirror_prefixes: []
```

#### E2. [src/hooks/useClientInstaller.test.tsx](file:///e:/Coding/DDNet/DDNet-Manager/src/hooks/useClientInstaller.test.tsx)

`baseSettings`（L42-53）末尾追加 `extra_trusted_hosts: []` 和 `mirror_prefixes: []`。

#### E3. [src/hooks/useAppUpdater.test.tsx](file:///e:/Coding/DDNet/DDNet-Manager/src/hooks/useAppUpdater.test.tsx)

`baseSettings`（L26-37）末尾追加 `extra_trusted_hosts: []` 和 `mirror_prefixes: []`。

#### E4. [src/lib/settings.test.ts](file:///e:/Coding/DDNet/DDNet/DDNet-Manager/src/lib/settings.test.ts)

在现有 `describe("updateNetworkRoute")` 内新增一个测试：
```typescript
it("stores auto_detect mode without requiring a proxy url", () => {
  const next = updateNetworkRoute(defaultAppSettings, "auto_detect", "http://127.0.0.1:7890");
  expect(next.network_route).toEqual({
    mode: "auto_detect",
    local_proxy_url: null
  });
});
```
**为什么**：验证 `auto_detect` 模式忽略传入的 URL，`local_proxy_url` 恒为 `null`。

#### E5. [src/lib/updateLogic.test.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.test.ts)

在 `describe("buildNetworkRoute")` 内新增：
```typescript
it("builds an auto_detect route without requiring a url", () => {
  expect(buildNetworkRoute("auto_detect", "")).toEqual({
    mode: "auto_detect",
    local_proxy_url: null
  });
});

it("exposes label and hint for auto_detect mode", () => {
  expect(networkRouteLabel("auto_detect")).toBe("自动检测");
  expect(networkRouteHint("auto_detect")).toContain("系统代理");
  expect(networkRoutePlaceholder("auto_detect")).toBe("");
});
```

### F. 门禁验收

#### F1. 运行 `make check-lint`

预期全绿。重点关注：
- `cargo fmt --check`：新增代码已 fmt。
- `cargo clippy -- -D warnings`：无告警。
- `cargo test`：mirror / race / network_route 全过。
- `bun run check`：TS 编译通过（`auto_detect` switch 穷尽、AppSettings 字段齐全）。
- 结构扫描：单文件/函数规模、`unwrap` 扫描（测试代码允许）、`mod.rs` 禁用等。

## 假设与决策

### 决策

1. **不改默认 mode**：`defaultAppSettings.network_route` 保持 `null`（即 `direct`），与后端 `AppSettings::default()` 一致。`auto_detect` 作为可选 mode 提供，不设为新默认。**理由**：改默认需同步改后端 `AppSettings::default()`，会重开已闭合的后端改动；且 `direct` 是最保守的默认，`auto_detect` 让用户主动选。原 P0 计划提过"默认选中"，本计划保守不改，如需改默认作为后续独立改动。
2. **新字段在 TS 中为必填**（非 `?:`）：与 Rust `Vec<String>` 契约一致，避免 `string[] | undefined` 的处理负担；代价是 3 处测试 mock 需补字段（已在 E1-E3 覆盖）。
3. **`auto_detect` 模式下 `local_proxy_url` 恒为 `null`**：前端不传 URL，后端 `resolve_effective_proxy` 走 `detect_system_proxy` 分支。
4. **env 测试用 `Mutex` 串行化**而非 `serial_test` crate：避免新增依赖；静态 `Mutex` + `_guard` 模式是 Rust 测试社区常见做法。
5. **UI mode 顺序** `direct` → `auto_detect` → `local_proxy`：从简到繁，`auto_detect` 居中作为推荐位。

### 假设

- 前序会话已修复的 mirror/race bug 在 A1 修完后不再复发；若复现就地在 A2 修复。
- `winreg` 在 Windows target 编译无问题；非 Windows 平台 `detect_windows_registry_proxy` 不编译，env 测试在任意平台可跑。
- 现有 `MAX_CONCURRENT_DOWNLOADS = 3` 与竞速并发（候选数 < 8）不冲突——竞速在单次下载任务内部，不占 `DownloadManager` 任务槽。
- 旧 SQLite 设置 JSON 反序列化兼容：serde 对缺失的 `extra_trusted_hosts`/`mirror_prefixes` 用 `Vec::default()`（空），已在前序会话验证。

## 验证步骤

1. `cd src-tauri && cargo test` —— 确认 mirror / race / network_route / net / registry 测试全过（重点看 env 测试串行化后是否绿）。
2. `make check-lint` —— 全绿（fmt + clippy + cargo test + bun install + bun run check + 结构扫描）。
3. （可选，手动）`make tauri-dev` —— 设置页确认网络路由出现三选一按钮（直连 / 自动检测 / 手动填写），选"自动检测"时地址输入框隐藏，选"手动填写"时输入框出现。

## 不在本次范围

- P1：反代列表外部化热更新（仍用 `DEFAULT_MIRROR_PREFIXES` 常量 + `AppSettings.mirror_prefixes` 覆盖）。
- P1：测速结果短期缓存。
- P1：前端竞速状态/加速源实时展示。
- P2：`.partial` 完整断点续传。
- 技术债：`Result<_, String>` → `thiserror` 类型化错误。
- 改默认 mode 为 `auto_detect`（见决策 1）。
