# 实现计划：下载加速与多源竞速（P0 + 代理配置）

基于 `docs/superpowers/specs/2026-06-24-下载加速与多源竞速方案.md`，本次实现 **P0 核心竞速链路 + 代理配置项**。P1（反代列表外部化热更新、测速缓存、前端竞速状态展示）和 P2（完整断点续传）留后续迭代。

## 范围与决策

### 本次实现

* P0：下载白名单可配置 + 反代域名动态放行（保留 SSRF 防护）

* P0：竞速测速模块（HEAD 淘汰 + Range 5 MiB 测吞吐）

* P0：下载执行层接入竞速选源 + 测速 chunk 复用 `.partial`

* P0：`resolve_asset_url` 反代前缀拼装层

* 代理配置：自动检测（**环境变量 + Windows 系统注册表**）+ 手动填写

### 不在本次范围

* P1：公共反代列表外部化热更新（本次用代码内 `DEFAULT_MIRROR_PREFIXES` 常量，`AppSettings.mirror_prefixes` 字段预留可覆盖）

* P1：测速结果短期缓存

* P1：前端加速源/竞速状态展示

* P2：`.partial` 完整断点续传

* 技术债 `thiserror` 类型化错误

### 关键技术决策

1. **候选 URL 列表引入方式**：`DownloadFileRequest` 新增 `candidate_urls: Vec<String>` 字段（含原始 GitHub URL + 反代 URL）；`asset_url` 保留为权威原始 URL 不变。竞速在下载层入口进行，保证"测速字节复用为 .partial 首段"可行。
2. **反代列表 P0 策略**：`mirror.rs` 内置 `DEFAULT_MIRROR_PREFIXES` 常量；`AppSettings.mirror_prefixes` 字段预留（空时用默认），P1 再加外部化拉取。
3. **代理模式枚举**：`NetworkRouteMode` 新增 `AutoDetect` 变体，保留 `Direct` / `LocalProxy`（UI 标签改为"手动填写"）以向后兼容旧设置。
4. **自动检测顺序**：先读 env `HTTPS_PROXY`/`https_proxy`/`HTTP_PROXY`/`http_proxy`；若为空，读 Windows 注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` 的 `ProxyEnable`(DWORD) + `ProxyServer`(String)。
5. **测速 chunk 复用**：竞速模块返回 `RaceWinner { url, head_start: Vec<u8> }`（5 MiB 字节），不接触文件系统；下载层把 `head_start` 写入 `.part` 后从 offset 5 MiB 用 `Range` 续传。
6. **SSRF 防护**：保留域名级白名单 + 拒绝 IP 直连（现有 `local_smoke::validate_public_ip` 已做私网拒绝）；`TRUSTED_DOWNLOAD_HOSTS` 保留为基线常量，新增 `is_host_trusted(host, extra)` 函数支持动态放行。

## 当前状态分析

### 已具备（无需改）

* `UpdateAsset`（[models.rs:521-531](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs#L521-L531)）已含 `size: u64` 和 `sha256: String`，文档说的 `asset_size` 实际叫 `size`。

* `verify_downloaded_file_with_progress`（[download/verify.rs:172](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/verify.rs#L172)）已具备，统一基准校验复用。

* `.part` + rename 模式（[download/net.rs:180-251](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L180-L251)）已具备 chunk 级 cancel、size 守卫、原子 rename。

* `network_route::build_routed_client`（[network\_route.rs:21](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/network_route.rs#L21)）已支持代理注入。

* 前端 IPC 集中封装在 [src/lib/tauri.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/tauri.ts)，无裸 invoke 散落。

### 需改造

* `TRUSTED_DOWNLOAD_HOSTS`（[download/net.rs:17-23](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L17-L23)）硬编码，无法放行反代域名。

* `validate_download_url`（[download/net.rs:32-78](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L32-L78)）不接受额外 host 列表参数。

* `DownloadFileRequest`（[download.rs:79-88](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download.rs#L79-L88)）只有单 `asset_url`，无候选 URL 概念。

* `stream_download_to_partfile`（[download/net.rs:180-241](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L180-L241)）不支持从指定 offset 续传。

* `NetworkRouteMode`（[models.rs:199-205](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs#L199-L205)）只有 `Direct`/`LocalProxy`，无 `AutoDetect`。

* `build_routed_client` 不解析 `mode` 字段，只看 `local_proxy_url` 是否非空。

* 前端网络路由 UI 内嵌在 [UpdatePanel.tsx:801-852](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.tsx#L801-L852)，需加 AutoDetect 选项。

## 改动清单

### 后端

#### 1. 新增 `src-tauri/src/mirror.rs`（\~50 行）

反代前缀拼装层，仅执行层使用。

```rust
/// 反代前缀拼装：把原始 GitHub URL 转为反代 URL（rustup 风格字符串拼接）。
pub fn resolve_asset_url(original: &str, proxy_prefix: Option<&str>) -> String { ... }

/// 由原始 URL + 多个反代前缀组装候选 URL 列表（原始 URL 始终首位）。
pub fn build_candidate_urls(original: &str, prefixes: &[String]) -> Vec<String> { ... }

/// P0 代码内 fallback 反代前缀列表（P1 外部化后此常量作为兜底）。
pub const DEFAULT_MIRROR_PREFIXES: &[&str] = &[
    "https://gh-proxy.com/",
    "https://mirror.ghproxy.com/",
    "https://gh.api.99988866.xyz/",
    "https://g.ioiox.com/",
];
```

单测：拼装正确性、prefix 为空、原始 URL 已含 path、trim\_end\_matches('/') 行为。

#### 2. 新增 `src-tauri/src/download/race.rs`（\~150 行）

竞速测速模块。在 `download.rs` 顶层 `pub mod race;` 声明。

```rust
/// 竞速胜出源：选中 URL + 已下载的 5 MiB 首段字节（用于复用为 .partial）。
pub struct RaceWinner {
    pub url: String,
    pub head_start: Vec<u8>,  // 长度 <= 5 MiB
}

/// 对候选源池并发竞速：阶段1 HEAD 淘汰 → 阶段2 Range 5 MiB 测吞吐 → 选最快。
/// 失败重试用指数退避（200ms 起倍增，上限 3s）。
pub async fn select_best_source(
    client: &reqwest::Client,
    candidate_urls: &[String],
    expected_size: u64,
    cancel: CancellationToken,
) -> Result<RaceWinner, String> { ... }

// 阶段1：并发 HEAD 淘汰不可达/不支持 Range/size 不符
async fn head_probe(client: &reqwest::Client, url: &str, expected_size: u64) -> Result<bool, String> { ... }

// 阶段2：并发 Range 下载前 5 MiB 测吞吐
const RACE_PROBE_BYTES: usize = 5 * 1024 * 1024;  // 5 MiB
const RACE_CONCURRENCY: usize = 8;
const RACE_HEAD_TIMEOUT: Duration = Duration::from_secs(5);
const RACE_RANGE_TIMEOUT: Duration = Duration::from_secs(10);
```

单测：用 `mockito` 或本地 HTTP server mock HEAD 200/403/无 Range 支持/size 不符；mock Range 下载快慢差异验证选最快；cancel token 中断。

#### 3. 改造 `src-tauri/src/download/net.rs`（\~80 行改动）

**3a. 白名单可配置化**：

* `TRUSTED_DOWNLOAD_HOSTS` 保留为基线 `const`。

* 新增 `pub fn is_host_trusted(host: &str, extra_hosts: &[String]) -> bool`：命中基线或 extra 即可信。

* `validate_download_url` 签名扩展：增加 `extra_hosts: &[String]` 参数，第 72-76 行的命中检查改为调 `is_host_trusted`。

* 保留现有 SSRF 防护（拒绝 localhost / 私网 IP / 歧义数字 host）不动。

**3b. 下载主流程接入竞速**：

* `download_asset_to_file`（[net.rs:127-138](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L127-L138)）签名扩展：增加 `candidate_urls: &[String]` 和 `extra_hosts: &[String]` 参数。

* 流程改为：

  1. 调 `race::select_best_source` 竞速，得到 `RaceWinner { url, head_start }`。
  2. 对 winner.url 调 `validate_download_url(url, extra_hosts)`。
  3. 创建 `.part` 文件，先写入 `head_start` 字节。
  4. 用 `Range: bytes={head_start.len()}-{expected_size-1}` 续传剩余部分到 `.part`。
  5. `finalize_download_cache` rename 不变。

* `stream_download_to_partfile`（[net.rs:180-241](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L180-L241)）扩展：增加 `start_offset: u64` 参数，从该 offset 追加写（`OpenOptions::new().append(true)` 或 seek），size 守卫改为 `total_written + start_offset <= expected_size`。

* `prepare_download_partfile`（[net.rs:141-177](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs#L141-L177)）拆分：URL 校验 + client 构建保留；重定向跟随 + Content-Length 比对移到竞速胜出后对 winner.url 单独做。

单测：extra\_hosts 放行新域名；head\_start 写入后续传；start\_offset 守卫溢出报错。

#### 4. 改造 `src-tauri/src/models.rs`（\~30 行改动）

* `NetworkRouteMode`（[models.rs:199-205](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs#L199-L205)）增加 `AutoDetect` 变体。`#[serde(rename_all = "snake_case")]` 保持。

* `NetworkRouteConfig`（[models.rs:208-214](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs#L208-L214)）字段不变（`mode` + `local_proxy_url`）。

* `AppSettings`（[models.rs:227-258](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs#L227-L258)）新增两字段：

  * `extra_trusted_hosts: Vec<String>`（默认空）

  * `mirror_prefixes: Vec<String>`（默认空，空时用 `DEFAULT_MIRROR_PREFIXES`）

* `AppSettings::Default`（[models.rs:282-297](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs#L282-L297)）补充新字段默认值。

* `DownloadFileRequest`（[download.rs:79-88](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download.rs#L79-L88)）新增 `candidate_urls: Vec<String>` 和 `extra_hosts: Vec<String>` 字段。

向后兼容：`serde` 默认对缺失字段用 `Default`，旧 SQLite 设置 JSON 反序列化不会失败。

#### 5. 改造 `src-tauri/src/network_route.rs`（\~100 行改动）

* 新增 `pub fn detect_system_proxy() -> Option<String>`：

  * 先读 env `HTTPS_PROXY` / `https_proxy` / `HTTP_PROXY` / `http_proxy`，非空直接返回。

  * 若为空且平台为 Windows，读注册表 `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Internet Settings`：

    * `ProxyEnable: DWORD` 为 1 时继续。

    * `ProxyServer: String` 格式可能为 `host:port` 或 `http=host:port;https=host:port`，解析出 https 代理（或首个）。

    * 返回 `http://host:port` 格式字符串。

  * 非 Windows 平台 env 为空则返回 `None`。

* 新增 `pub fn resolve_effective_proxy(route: Option<&NetworkRouteConfig>) -> Option<String>`：

  * `mode == Direct` → `None`

  * `mode == LocalProxy` → `route.local_proxy_url.clone()`

  * `mode == AutoDetect` → `detect_system_proxy()`

* `build_routed_client`（[network\_route.rs:21-26](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/network_route.rs#L21-L26)）改造：第 39-48 行的代理注入改为调 `resolve_effective_proxy(route)`，而非直接读 `local_proxy_url`。

* Windows 注册表读取用 `winreg` crate（轻量，仅 Windows API 绑定）。`Cargo.toml` 增加 `winreg = "0.52"`，target 为 Windows 时生效。

单测：env 变量检测（设 env 后断言）；注册表检测在 CI 难以模拟，用 `#[cfg(windows)]` 条件编译 + 手动验证，或抽象 trait 后 mock。

#### 6. 改造 `src-tauri/src/commands/download.rs`（\~40 行改动）

* `run_download_loop`（[commands/download.rs:276-317](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/commands/download.rs#L276-L317)）中构造 `DownloadFileRequest` 处（第 279-281 行）：

  * 用 `mirror::build_candidate_urls(&job.asset_url, &mirror_prefixes)` 生成 `candidate_urls`。

  * `mirror_prefixes` 来源：从 `AppSettings.mirror_prefixes` 读，空则用 `DEFAULT_MIRROR_PREFIXES`。

  * `extra_hosts` 来源：从 `AppSettings.extra_trusted_hosts` 读。

  * `DownloadFileRequest` 填入 `candidate_urls` 和 `extra_hosts`。

* `create_download_job`（[download.rs:233-258](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download.rs#L233-L258)）需能拿到 settings 或由调用方传入 mirror\_prefixes/extra\_hosts。决策：在 `start_update_download` command 入口加载 `AppSettings`，把 mirror\_prefixes + extra\_hosts 透传到 `run_download_loop`。

#### 7. `src-tauri/src/main.rs` 无需改

`generate_handler!`（[main.rs:109-138](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/main.rs#L109-L138)）无新命令（P0 复用 `save_app_settings` 持久化新字段）。

#### 8. `src-tauri/Cargo.toml`

新增 `winreg = { version = "0.52", optional = true }` + `[target.'cfg(windows)'.dependencies]` 启用。

### 前端

#### 9. 改造 `src/types.ts`（\~15 行改动）

* `NetworkRouteMode`（[types.ts:140-158](file:///e:/Coding/DDNet/DDNet-Manager/src/types.ts#L140-L158)）增加 `"auto_detect"` 字面量。

* `AppSettings` 增加 `extra_trusted_hosts: string[]` 和 `mirror_prefixes: string[]`。

#### 10. 改造 `src/components/update/UpdatePanel.tsx`（\~40 行改动）

* 网络路由 UI（[UpdatePanel.tsx:801-852](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.tsx#L801-L852)）的 mode 选择从二选一改为三选一：

  * `direct`（直连）

  * `auto_detect`（自动检测系统代理）—— 新增，默认选中

  * `local_proxy`（手动填写）—— 原 `local_proxy`，标签改为"手动填写"

* `auto_detect` 模式下隐藏代理地址输入框；`local_proxy` 模式下显示输入框（保留现有 `127.0.0.1:7890` 占位提示）。

* `updateNetworkRoute`（[src/lib/settings.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/settings.ts)）逻辑同步：mode 切换时清空/保留 `local_proxy_url` 的策略不变。

* `buildStartUpdateDownloadRequest` / `buildUpdateSourceRequest`（[src/lib/updateLogic.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.ts)）同步新 mode 字面量。

### 测试

#### 11. 后端单测

* `mirror.rs`：URL 拼装、空 prefix、trim slash、默认列表非空。

* `download/race.rs`：mockito mock HEAD/Range，验证淘汰逻辑、选最快、cancel、并发上限。

* `download/net.rs`：`is_host_trusted` 命中/未命中；`stream_download_to_partfile` 的 `start_offset` 续传 + size 守卫溢出报错。

* `network_route.rs`：`detect_system_proxy` env 分支（设 env 断言）；`resolve_effective_proxy` 三种 mode 分派。

#### 12. 前端测试

* `UpdatePanel.test.tsx`：新增 AutoDetect 选项渲染、切换 mode 时输入框显隐、保存设置 payload 含新字段。

## 验证步骤

1. `make check-lint` 全绿（fmt + clippy + test + TS + 结构扫描）。
2. 手动验证（Windows）：

   * 启动 Clash，设置系统代理 `127.0.0.1:7890`。

   * DDNet Manager 设置页选"自动检测"，触发 QmClient 更新下载。

   * 观察日志：竞速模块应检测到 `127.0.0.1:7890`，GitHub 直连候选应胜出（挂代理场景）。

   * 关闭 Clash 系统代理，选"自动检测"重试，反代候选应胜出（裸连场景）。

   * 选"手动填写"填入错误地址，验证 fallback 到反代候选或报错。
3. SHA-256 校验：故意篡改反代前缀指向本地 mock server 返回错误内容，验证下载后校验失败、删缓存、不自动重试。

## 假设与风险

### 假设

* `AppSettings` 新字段对旧 SQLite 行的反序列化兼容（serde 缺失字段用 Default，已确认无 `#[serde(deny_unknown_fields)]`）。

* `winreg` crate 在 Windows target 编译无问题；非 Windows 平台 `detect_system_proxy` 仅走 env 分支。

* 现有 `MAX_CONCURRENT_DOWNLOADS = 3`（[download.rs:102](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download.rs#L102)）不与竞速并发（8）冲突——竞速在单次下载任务内部，不占 `DownloadManager` 的任务槽。

### 风险

1. **Windows 注册表** **`ProxyServer`** **格式多样**：可能是 `host:port` 或 `http=host:port;https=host:port;ftp=...`。解析需覆盖两种，单元测试用字符串解析 mock。
2. **反代前缀列表 P0 硬编码易失效**：P0 用代码内常量，失效需发版修复；P1 外部化后此风险消除。文档已记为已知风险。
3. **竞速阶段 5 MiB 写入** **`.part`** **后下载失败**：`.part` 残留。对策：下载失败时清理 `.part`（现有 `stream_download_to_partfile` 已在 cancel/溢出时清理，竞速胜出后复用同一清理路径）。
4. **测速阶段反代返回 200 但实际下载慢**：阶段2 硬超时 10s 淘汰，已覆盖。

