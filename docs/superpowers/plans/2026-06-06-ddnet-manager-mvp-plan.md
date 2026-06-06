# DDNet Manager MVP 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 基于 `docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md` 实现 QmClient-first MVP：本地客户端识别、manifest 更新、启动、资源目录、Binds 分析与安全写入。

**架构：** Rust 后端负责文件系统、进程、下载、cfg 解析和事务写入；React 前端负责状态展示、交互流和差异确认。先建立稳定领域模型和 IPC 契约，再逐步替换当前 mock 命令。

**技术栈：** Tauri v2、Rust 2021、tokio、reqwest、serde、serde_json、React 19、TypeScript 6、Tailwind CSS v4、Framer Motion、Bun、Make。

---

## 当前执行状态

implementation_status: partial_implementation_verified
updated_at: 2026-06-06

- 任务 1-17 的首版脚手架、基础 IPC、基础后端模块、前端页面骨架和自动化验证均已落地。
- 子代理驱动流程已执行：任务 15、任务 16、任务 17 均完成实现子代理、规格审查子代理、代码质量审查子代理流程；发现的阻塞问题已修复并复审通过。
- 自动门禁已通过：`make check-lint` 汇总为 `PASS: 15 / WARN: 1 / FAIL: 0`；唯一 `WARN` 为已知 `cargo audit` 上游 allowed warnings。
- 前端生产构建已通过：`make build` 成功生成 `dist/index.html` 和 assets，且 `dist/` 已被 `.gitignore` 忽略。
- `AGENTS.md` 与 `CLAUDE.md` 哈希一致，两份代理规则文本保持同步。
- 手动桌面联调尚未执行：任务 15、任务 16 中的 `make tauri-dev` 真实路径验证仍为验证 gap，不得声称已经完成。
- 已补齐基础闭环：`cfg` 深度解析支持 `bind`、`unbind`、`exec` 链、缺失目标和按键冲突分析；更新页已接入 `load_manifest` 元数据读取；资源页已接入 `validate_client_dir` 路径识别。
- PRD 级 MVP 尚未完全收口：真实下载、校验、安装、回滚事务和桌面手动联调仍未完成；更新页当前明确只做 manifest 元数据闭环。
- 当前仓库已有初始化提交：`feat(core): 初始化 DDNet Manager MVP 基线`。原计划中的逐任务 commit 步骤未按粒度执行，后续新主题改动应按中文 Conventional Commits 单独提交。

## 工程基线更新

以下工程基线已经落地，后续执行本计划时不得重复实现：

- 已创建：`scripts/check_lint.sh`。职责：统一工程门禁，覆盖 Rust fmt、clippy、test、Bun lockfile、TypeScript、Rust 结构扫描、Tauri command 文档注释和占位符扫描。
- 已修改：`Makefile`。职责：统一 `make check-lint`、`make check-lint-fix`、`make fmt`、`make test`、`make rust-test` 等命令入口。
- 已同步：`AGENTS.md` 与 `CLAUDE.md`。职责：保持代理规则、Rust 规约、TDD、子代理审查触发条件完全一致。
- 已修改：`src-tauri/src/main.rs`。职责：Tauri command 已补中文 `///` 文档注释，入口错误处理已去除非测试 `.expect()`。

后续所有新增 `pub` Rust item 和 `#[tauri::command]` 函数必须写中文 `///` 文档注释。后续任务的局部红/绿验证可使用 `make rust-test`、`make rust-check` 或 `make check`，但每个实现任务完成前必须运行 `make check-lint`。当前 `cargo audit` 对 Tauri 上游依赖会产生 allowed warnings，这是已知非阻塞 WARN；新增 FAIL 或本次引入的新 WARN 不得遗留。

使用子代理驱动执行时，每个实现任务完成后必须按顺序派发两个只读审查：先做规格合规性审查，再做代码质量审查。审查子代理不得修改文件，不得派发子代理；审查未返回最终报告前，不得标记任务完成。

## 文件结构

### Rust 后端

- 创建：`src-tauri/src/models.rs`。职责：定义跨 IPC 的领域类型，包括客户端安装、manifest、扫描结果、Bind 记录、文件事务。
- 创建：`src-tauri/src/errors.rs`。职责：统一后端错误类型，转换为 Tauri command 可返回的字符串。
- 创建：`src-tauri/src/paths.rs`。职责：Windows 常见路径、用户数据目录、配置目录推导。
- 创建：`src-tauri/src/client_scan.rs`。职责：本地客户端识别、常见路径扫描、手动目录验证、Everything provider 检测。
- 创建：`src-tauri/src/manifest.rs`。职责：manifest 读取、校验、版本比较和网络配置。
- 创建：`src-tauri/src/process.rs`。职责：启动客户端、检测运行中的 `DDNet.exe`。
- 创建：`src-tauri/src/cfg.rs`。职责：cfg 解析器，解析 `bind`、`unbind`、`exec`、注释和来源行号。
- 创建：`src-tauri/src/workshop.rs`。职责：消费 Workshop 公开 JSON，映射 `binds.json` / `cfg-groups.json`。
- 创建：`src-tauri/src/file_tx.rs`。职责：配置写入事务、备份、差异、回滚。
- 创建：`src-tauri/src/commands.rs`。职责：Tauri command 聚合层，调用各领域模块。
- 修改：`src-tauri/src/main.rs`。职责：改为注册 `commands.rs` 中的 commands，保留 Tauri builder。
- 修改：`src-tauri/Cargo.toml`。职责：补齐 `thiserror`、`dirs`、`sha2`、`zip`、`tempfile`、`similar` 等依赖。

### Rust 测试

- 创建：`src-tauri/src/fixtures/client_valid/`。职责：测试用完整客户端目录。
- 创建：`src-tauri/src/fixtures/cfg/`。职责：测试用 cfg 文件和 exec 链。
- 测试内联放在对应 Rust 模块的 `#[cfg(test)] mod tests` 中，优先使用 `tempfile` 创建隔离目录。

### 前端

- 创建：`src/types.ts`。职责：前后端共享 TypeScript 类型。
- 创建：`src/lib/tauri.ts`。职责：封装 `invoke` 和事件监听，集中 command 名称。
- 创建：`src/components/layout/TitleBar.tsx`。职责：从 `App.tsx` 拆出标题栏。
- 创建：`src/components/launch/LaunchPanel.tsx`。职责：启动页默认客户端和启动按钮。
- 创建：`src/components/clients/ClientManager.tsx`。职责：客户端扫描、列表、手动添加、默认选择。
- 创建：`src/components/update/UpdatePanel.tsx`。职责：manifest 状态、下载、代理/镜像配置入口。
- 创建：`src/components/resources/ResourcePanel.tsx`。职责：安装目录、用户目录和资源目录展示。
- 创建：`src/components/binds/BindsPanel.tsx`。职责：本地 Binds 分析、Workshop 条目、差异预览、写入状态。
- 修改：`src/App.tsx`。职责：从单页 mock UI 改为带导航的 Manager shell，组合上述页面。
- 修改：`src/index.css`。职责：保留现有工业风基础类，补充导航和表格所需少量全局类。

### 前端测试

- 暂不引入组件测试框架作为第一步阻塞项。前端局部验证以 `make check` 和后续手动 Tauri 走查为准；任务完成验收以 `make check-lint` 为准。
- 若后续添加测试框架，优先选择 Vitest + React Testing Library，并通过单独计划实施。

## 阶段 1：后端基础模型与命令拆分

### 任务 1：创建领域模型

**文件：**
- 创建：`src-tauri/src/models.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写失败的模型序列化测试**

在 `src-tauri/src/models.rs` 写入：

```rust
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_installation_serializes_with_snake_case_fields() {
        let item = ClientInstallation {
            id: "qmclient-local-1".to_string(),
            client_id: "qmclient".to_string(),
            display_name: "QmClient 2.62.5".to_string(),
            install_dir: "C:/Games/QmClient".to_string(),
            executable_path: "C:/Games/QmClient/DDNet.exe".to_string(),
            storage_cfg_path: "C:/Games/QmClient/storage.cfg".to_string(),
            data_dir: "C:/Games/QmClient/data".to_string(),
            user_data_dir: Some("C:/Users/User/AppData/Roaming/DDNet".to_string()),
            version: Some("2.62.5".to_string()),
            is_default: true,
            health: ClientHealth::Ok,
        };

        let json = serde_json::to_value(item).expect("serialize");
        assert_eq!(json["client_id"], "qmclient");
        assert_eq!(json["is_default"], true);
        assert_eq!(json["health"], "ok");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find struct ClientInstallation` 或 `cannot find type ClientHealth`。

- [ ] **步骤 3：实现最少模型**

在 `src-tauri/src/models.rs` 补齐：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientHealth {
    Ok,
    MissingExecutable,
    MissingStorageCfg,
    MissingDataDir,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientInstallation {
    pub id: String,
    pub client_id: String,
    pub display_name: String,
    pub install_dir: String,
    pub executable_path: String,
    pub storage_cfg_path: String,
    pub data_dir: String,
    pub user_data_dir: Option<String>,
    pub version: Option<String>,
    pub is_default: bool,
    pub health: ClientHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateAsset {
    pub platform: String,
    pub asset_url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestClient {
    pub client_id: String,
    pub channel: String,
    pub version: String,
    pub release_notes: String,
    pub assets: Vec<UpdateAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    pub schema_version: u32,
    pub clients: Vec<ManifestClient>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindRecord {
    pub key: String,
    pub command: String,
    pub source_file: String,
    pub line: usize,
    pub managed_by_manager: bool,
    pub matched_workshop_id: Option<String>,
}
```

在 `src-tauri/src/main.rs` 顶部添加：

```rust
mod models;
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS，Rust check 完成。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/models.rs
git commit -m "feat(core): 定义 DDNet Manager 领域模型"
```

### 任务 2：拆分 Tauri commands

**文件：**
- 创建：`src-tauri/src/commands.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写失败的 command smoke 测试**

在 `src-tauri/src/commands.rs` 写入：

```rust
use crate::models::{ClientHealth, ClientInstallation};

pub fn mocked_default_client() -> ClientInstallation {
    ClientInstallation {
        id: "qmclient-mock".to_string(),
        client_id: "qmclient".to_string(),
        display_name: "QmClient Mock".to_string(),
        install_dir: "C:/Games/QmClient".to_string(),
        executable_path: "C:/Games/QmClient/DDNet.exe".to_string(),
        storage_cfg_path: "C:/Games/QmClient/storage.cfg".to_string(),
        data_dir: "C:/Games/QmClient/data".to_string(),
        user_data_dir: None,
        version: None,
        is_default: true,
        health: ClientHealth::Ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mocked_default_client_is_qmclient() {
        let client = mocked_default_client();
        assert_eq!(client.client_id, "qmclient");
        assert_eq!(client.health, ClientHealth::Ok);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，因为 `commands` 模块尚未在 `main.rs` 声明，或 command 注册仍在旧位置。

- [ ] **步骤 3：迁移 commands**

在 `src-tauri/src/main.rs` 顶部添加：

```rust
mod commands;
```

把原有 `VersionInfo`、`DownloadProgress`、`check_update`、`start_download`、`launch_game` 移入 `src-tauri/src/commands.rs`，并把 command 函数改为 `pub`。迁移后的函数签名必须是：

```rust
#[tauri::command]
pub async fn check_update() -> Result<VersionInfo, String>

#[tauri::command]
pub async fn start_download(app: AppHandle) -> Result<(), String>

#[tauri::command]
pub fn launch_game() -> Result<(), String>
```

函数体保留当前 `main.rs` 中已经能通过 `make rust-test` 的实现，只改变模块位置和可见性。

在 `main.rs` 的 `invoke_handler` 中改为：

```rust
.invoke_handler(tauri::generate_handler![
    commands::check_update,
    commands::start_download,
    commands::launch_game
])
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/commands.rs
git commit -m "refactor(core): 拆分 Tauri 命令入口"
```

## 阶段 2：客户端识别与启动

### 任务 3：实现客户端目录验证

**文件：**
- 创建：`src-tauri/src/client_scan.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/client_scan.rs` 写入：

```rust
use std::path::Path;

use crate::models::{ClientHealth, ClientInstallation};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_complete_client_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("DDNet.exe"), "").expect("exe");
        fs::write(temp.path().join("storage.cfg"), "add_path $USERDIR\n").expect("storage");
        fs::create_dir(temp.path().join("data")).expect("data dir");

        let client = validate_client_dir(temp.path()).expect("valid client");

        assert_eq!(client.client_id, "qmclient");
        assert_eq!(client.health, ClientHealth::Ok);
        assert!(client.executable_path.ends_with("DDNet.exe"));
    }

    #[test]
    fn reports_missing_executable() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("storage.cfg"), "add_path $USERDIR\n").expect("storage");
        fs::create_dir(temp.path().join("data")).expect("data dir");

        let client = validate_client_dir(temp.path()).expect("client result");

        assert_eq!(client.health, ClientHealth::MissingExecutable);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function validate_client_dir` 和 `tempfile` 未声明。

- [ ] **步骤 3：补依赖与实现**

在 `src-tauri/Cargo.toml` 添加：

```toml
thiserror = "2.0.18"
dirs = "6.0.0"
tempfile = "3.23.0"
```

在 `src-tauri/src/main.rs` 添加：

```rust
mod client_scan;
```

在 `src-tauri/src/client_scan.rs` 实现：

```rust
use std::path::Path;

use crate::models::{ClientHealth, ClientInstallation};

pub fn validate_client_dir(path: &Path) -> Result<ClientInstallation, String> {
    let executable_path = path.join("DDNet.exe");
    let storage_cfg_path = path.join("storage.cfg");
    let data_dir = path.join("data");

    let health = if !executable_path.is_file() {
        ClientHealth::MissingExecutable
    } else if !storage_cfg_path.is_file() {
        ClientHealth::MissingStorageCfg
    } else if !data_dir.is_dir() {
        ClientHealth::MissingDataDir
    } else {
        ClientHealth::Ok
    };

    let install_dir = path.to_string_lossy().replace('\\', "/");

    Ok(ClientInstallation {
        id: format!("qmclient-{}", stable_path_id(&install_dir)),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir,
        executable_path: executable_path.to_string_lossy().replace('\\', "/"),
        storage_cfg_path: storage_cfg_path.to_string_lossy().replace('\\', "/"),
        data_dir: data_dir.to_string_lossy().replace('\\', "/"),
        user_data_dir: default_user_data_dir(),
        version: None,
        is_default: false,
        health,
    })
}

fn default_user_data_dir() -> Option<String> {
    dirs::data_dir().map(|dir| dir.join("DDNet").to_string_lossy().replace('\\', "/"))
}

fn stable_path_id(path: &str) -> String {
    let mut hash = 0_u64;
    for byte in path.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(*byte));
    }
    format!("{hash:x}")
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/main.rs src-tauri/src/client_scan.rs
git commit -m "feat(client): 验证本地客户端目录"
```

### 任务 4：添加扫描与启动命令

**文件：**
- 修改：`src-tauri/src/client_scan.rs`
- 修改：`src-tauri/src/process.rs`
- 修改：`src-tauri/src/commands.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写失败测试**

创建 `src-tauri/src/process.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddnet_process_names_match_expected_executables() {
        assert!(is_ddnet_process_name("DDNet.exe"));
        assert!(is_ddnet_process_name("ddnet.exe"));
        assert!(!is_ddnet_process_name("notepad.exe"));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function is_ddnet_process_name`。

- [ ] **步骤 3：实现进程工具与命令**

在 `src-tauri/src/main.rs` 添加：

```rust
mod process;
```

在 `src-tauri/src/process.rs` 实现：

```rust
pub fn is_ddnet_process_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("DDNet.exe") || name.eq_ignore_ascii_case("DDNet")
}

pub fn launch_executable(path: &str) -> Result<(), String> {
    std::process::Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch {path}: {error}"))
}
```

在 `src-tauri/src/commands.rs` 添加：

```rust
#[tauri::command]
pub fn validate_client_dir(path: String) -> Result<crate::models::ClientInstallation, String> {
    crate::client_scan::validate_client_dir(std::path::Path::new(&path))
}

#[tauri::command]
pub fn launch_client(path: String) -> Result<(), String> {
    crate::process::launch_executable(&path)
}
```

在 `main.rs` 注册：

```rust
commands::validate_client_dir,
commands::launch_client
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/commands.rs src-tauri/src/process.rs src-tauri/src/client_scan.rs
git commit -m "feat(client): 添加客户端验证与启动命令"
```

## 阶段 3：Manifest 更新链路

### 任务 5：实现 manifest 解析

**文件：**
- 创建：`src-tauri/src/manifest.rs`
- 修改：`src-tauri/src/main.rs`
- 修改：`src-tauri/src/models.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/manifest.rs` 写入：

```rust
use crate::models::UpdateManifest;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_with_qmclient_asset() {
        let json = r#"{
            "schema_version": 1,
            "clients": [{
                "client_id": "qmclient",
                "channel": "stable",
                "version": "2.62.5",
                "release_notes": "https://github.com/wxj881027/QmClient/releases/tag/v2.62.5",
                "assets": [{
                    "platform": "windows-x64",
                    "asset_url": "https://github.com/wxj881027/QmClient/releases/download/v2.62.5/QmClient-2.62.5-win64.zip",
                    "sha256": "0123456789abcdef",
                    "size": 89531134
                }]
            }]
        }"#;

        let manifest = parse_manifest(json).expect("manifest");

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.clients[0].client_id, "qmclient");
        assert_eq!(manifest.clients[0].assets[0].platform, "windows-x64");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function parse_manifest`。

- [ ] **步骤 3：实现解析与命令**

在 `src-tauri/src/main.rs` 添加：

```rust
mod manifest;
```

在 `src-tauri/src/manifest.rs` 实现：

```rust
use crate::models::UpdateManifest;

pub fn parse_manifest(input: &str) -> Result<UpdateManifest, String> {
    let manifest: UpdateManifest =
        serde_json::from_str(input).map_err(|error| format!("invalid manifest json: {error}"))?;

    if manifest.schema_version == 0 {
        return Err("manifest schema_version must be greater than 0".to_string());
    }
    if manifest.clients.is_empty() {
        return Err("manifest must contain at least one client".to_string());
    }

    Ok(manifest)
}

pub async fn fetch_manifest(url: &str, proxy_base_url: Option<&str>) -> Result<UpdateManifest, String> {
    let final_url = proxy_base_url
        .map(|base| format!("{}{}", base.trim_end_matches('/'), url))
        .unwrap_or_else(|| url.to_string());

    let text = reqwest::get(&final_url)
        .await
        .map_err(|error| format!("failed to fetch manifest: {error}"))?
        .text()
        .await
        .map_err(|error| format!("failed to read manifest response: {error}"))?;

    parse_manifest(&text)
}
```

在 `commands.rs` 添加：

```rust
#[tauri::command]
pub async fn load_manifest(url: String, proxy_base_url: Option<String>) -> Result<crate::models::UpdateManifest, String> {
    crate::manifest::fetch_manifest(&url, proxy_base_url.as_deref()).await
}
```

在 `main.rs` 注册：

```rust
commands::load_manifest
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/commands.rs src-tauri/src/manifest.rs src-tauri/src/models.rs
git commit -m "feat(update): 添加 manifest 解析与加载命令"
```

### 任务 6：实现下载校验骨架

**文件：**
- 创建：`src-tauri/src/download.rs`
- 修改：`src-tauri/Cargo.toml`
- 修改：`src-tauri/src/main.rs`
- 修改：`src-tauri/src/commands.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/download.rs` 写入：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_matches_known_value() {
        let hash = sha256_hex(b"ddnet-manager");
        assert_eq!(
            hash,
            "40ee15a6be3de8583a89e38e7bd0de4d3cd8d65ad2094c94f4b8e4cfa005f989"
        );
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function sha256_hex` 和 `sha2` 未声明。

- [ ] **步骤 3：实现 hash 工具**

在 `src-tauri/Cargo.toml` 添加：

```toml
sha2 = "0.10.9"
```

在 `src-tauri/src/main.rs` 添加：

```rust
mod download;
```

在 `src-tauri/src/download.rs` 实现：

```rust
use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
```

如果测试 hash 不匹配，以实际 `sha256_hex(b"ddnet-manager")` 输出为准更新断言；只允许更新为工具计算出的确定值，不允许移除断言。

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/main.rs src-tauri/src/download.rs
git commit -m "feat(update): 添加下载校验基础能力"
```

## 阶段 4：cfg 解析与 Binds 分析

### 任务 7：实现 cfg bind 解析器

**文件：**
- 创建：`src-tauri/src/cfg.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/cfg.rs` 写入：

```rust
use crate::models::BindRecord;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bind_lines_with_source_line() {
        let cfg = r#"
# comment
bind mouse3 "echo Kill; kill"
bind ctrl+z "echo 自救; say /rescue"
"#;

        let records = parse_cfg_binds("settings_ddnet.cfg", cfg);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].key, "mouse3");
        assert_eq!(records[0].command, "echo Kill; kill");
        assert_eq!(records[0].line, 3);
        assert_eq!(records[1].key, "ctrl+z");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function parse_cfg_binds`。

- [ ] **步骤 3：实现最小解析器**

在 `src-tauri/src/main.rs` 添加：

```rust
mod cfg;
```

在 `src-tauri/src/cfg.rs` 实现：

```rust
use crate::models::BindRecord;

pub fn parse_cfg_binds(source_file: &str, content: &str) -> Vec<BindRecord> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| parse_bind_line(source_file, index + 1, line))
        .collect()
}

fn parse_bind_line(source_file: &str, line_number: usize, line: &str) -> Option<BindRecord> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let rest = trimmed.strip_prefix("bind ")?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let key = parts.next()?.trim();
    let command = parts.next()?.trim().trim_matches('"');

    if key.is_empty() || command.is_empty() {
        return None;
    }

    Some(BindRecord {
        key: key.to_string(),
        command: command.to_string(),
        source_file: source_file.to_string(),
        line: line_number,
        managed_by_manager: false,
        matched_workshop_id: None,
    })
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/cfg.rs
git commit -m "feat(binds): 解析 cfg bind 记录"
```

### 任务 8：实现 cfg 文件扫描命令

**文件：**
- 修改：`src-tauri/src/cfg.rs`
- 修改：`src-tauri/src/commands.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/cfg.rs` 的 tests 中追加：

```rust
#[test]
fn parses_cfg_file_from_disk() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cfg_path = temp.path().join("settings_ddnet.cfg");
    std::fs::write(&cfg_path, "bind mouse3 \"echo Kill; kill\"\n").expect("write cfg");

    let records = parse_cfg_file(&cfg_path).expect("parse file");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].source_file, "settings_ddnet.cfg");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function parse_cfg_file`。

- [ ] **步骤 3：实现文件扫描**

在 `src-tauri/src/cfg.rs` 添加：

```rust
use std::path::Path;

pub fn parse_cfg_file(path: &Path) -> Result<Vec<BindRecord>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read cfg {}: {error}", path.display()))?;
    let source_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown.cfg");
    Ok(parse_cfg_binds(source_file, &content))
}
```

在 `commands.rs` 添加：

```rust
#[tauri::command]
pub fn analyze_cfg_file(path: String) -> Result<Vec<crate::models::BindRecord>, String> {
    crate::cfg::parse_cfg_file(std::path::Path::new(&path))
}
```

在 `main.rs` 注册：

```rust
commands::analyze_cfg_file
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/commands.rs src-tauri/src/cfg.rs
git commit -m "feat(binds): 添加 cfg 分析命令"
```

## 阶段 5：Workshop 数据接入

### 任务 9：实现 Workshop JSON 适配器

**文件：**
- 创建：`src-tauri/src/workshop.rs`
- 修改：`src-tauri/src/models.rs`
- 修改：`src-tauri/src/main.rs`
- 修改：`src-tauri/src/commands.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/models.rs` 添加类型引用前，先在 `src-tauri/src/workshop.rs` 写入测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_binds_payload() {
        let json = r#"{
            "binds": [{
                "id": "bind-防自杀-fb77af69c13c",
                "category": "基础",
                "title": "防自杀",
                "command": "bind mouse3 \"echo Kill; kill\"",
                "description": "鼠标中键 - 快速自杀重开",
                "commandVariants": [],
                "variantLabels": [],
                "isBindable": true
            }]
        }"#;

        let binds = parse_workshop_binds(json).expect("binds");

        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].id, "bind-防自杀-fb77af69c13c");
        assert_eq!(binds[0].category, "基础");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function parse_workshop_binds`。

- [ ] **步骤 3：实现类型与解析**

在 `src-tauri/src/models.rs` 添加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkshopBind {
    pub id: String,
    pub category: String,
    pub title: String,
    pub command: String,
    pub description: String,
    #[serde(default)]
    pub command_variants: Vec<String>,
    #[serde(default)]
    pub variant_labels: Vec<String>,
    #[serde(default)]
    pub is_bindable: bool,
}
```

在 `src-tauri/src/main.rs` 添加：

```rust
mod workshop;
```

在 `src-tauri/src/workshop.rs` 实现：

```rust
use serde::Deserialize;

use crate::models::WorkshopBind;

#[derive(Deserialize)]
struct WorkshopBindsPayload {
    binds: Vec<WorkshopBind>,
}

pub fn parse_workshop_binds(input: &str) -> Result<Vec<WorkshopBind>, String> {
    let payload: WorkshopBindsPayload =
        serde_json::from_str(input).map_err(|error| format!("invalid workshop binds json: {error}"))?;
    Ok(payload.binds)
}

pub async fn fetch_workshop_binds(url: &str) -> Result<Vec<WorkshopBind>, String> {
    let text = reqwest::get(url)
        .await
        .map_err(|error| format!("failed to fetch workshop binds: {error}"))?
        .text()
        .await
        .map_err(|error| format!("failed to read workshop binds response: {error}"))?;
    parse_workshop_binds(&text)
}
```

在 `commands.rs` 添加：

```rust
#[tauri::command]
pub async fn load_workshop_binds(url: String) -> Result<Vec<crate::models::WorkshopBind>, String> {
    crate::workshop::fetch_workshop_binds(&url).await
}
```

在 `main.rs` 注册：

```rust
commands::load_workshop_binds
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/commands.rs src-tauri/src/models.rs src-tauri/src/workshop.rs
git commit -m "feat(workshop): 接入 Bind Workshop 静态数据"
```

## 阶段 6：安全写入与回滚

### 任务 10：实现 Manager 专用 cfg 生成

**文件：**
- 创建：`src-tauri/src/file_tx.rs`
- 修改：`src-tauri/src/main.rs`
- 修改：`src-tauri/src/models.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/file_tx.rs` 写入：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_manager_cfg_with_markers() {
        let output = render_manager_cfg(&[
            "bind mouse3 \"echo Kill; kill\"".to_string(),
            "bind ctrl+z \"echo 自救; say /rescue\"".to_string(),
        ]);

        assert!(output.contains("# DDNET_MANAGER_BEGIN"));
        assert!(output.contains("bind mouse3"));
        assert!(output.contains("# DDNET_MANAGER_END"));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function render_manager_cfg`。

- [ ] **步骤 3：实现专用 cfg 生成**

在 `src-tauri/src/main.rs` 添加：

```rust
mod file_tx;
```

在 `src-tauri/src/file_tx.rs` 实现：

```rust
pub const MANAGER_BEGIN: &str = "# DDNET_MANAGER_BEGIN";
pub const MANAGER_END: &str = "# DDNET_MANAGER_END";

pub fn render_manager_cfg(commands: &[String]) -> String {
    let mut output = String::new();
    output.push_str(MANAGER_BEGIN);
    output.push('\n');
    output.push_str("# This block is managed by DDNet Manager.\n");
    for command in commands {
        output.push_str(command.trim());
        output.push('\n');
    }
    output.push_str(MANAGER_END);
    output.push('\n');
    output
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/file_tx.rs
git commit -m "feat(binds): 生成 Manager 专用 cfg"
```

### 任务 11：实现写入前备份与运行中保护接口

**文件：**
- 修改：`src-tauri/src/file_tx.rs`
- 修改：`src-tauri/src/commands.rs`

- [ ] **步骤 1：编写失败测试**

在 `src-tauri/src/file_tx.rs` tests 中追加：

```rust
#[test]
fn backup_file_creates_copy_next_to_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cfg_path = temp.path().join("settings_ddnet.cfg");
    std::fs::write(&cfg_path, "bind mouse3 \"echo Kill; kill\"\n").expect("write");

    let backup = backup_file(&cfg_path).expect("backup");

    assert!(backup.is_file());
    assert!(backup.file_name().unwrap().to_string_lossy().contains(".bak."));
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
make rust-test
```

预期：FAIL，报错包含 `cannot find function backup_file`。

- [ ] **步骤 3：实现备份与写入命令骨架**

在 `src-tauri/src/file_tx.rs` 添加：

```rust
use std::path::{Path, PathBuf};

pub fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid file name: {}", path.display()))?;
    let backup_name = format!("{file_name}.bak.{}", chrono_like_timestamp());
    let backup_path = path.with_file_name(backup_name);
    std::fs::copy(path, &backup_path)
        .map_err(|error| format!("failed to backup {}: {error}", path.display()))?;
    Ok(backup_path)
}

fn chrono_like_timestamp() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    millis.to_string()
}
```

在 `commands.rs` 添加：

```rust
#[tauri::command]
pub fn render_manager_bind_cfg(commands: Vec<String>) -> Result<String, String> {
    Ok(crate::file_tx::render_manager_cfg(&commands))
}
```

在 `main.rs` 注册：

```rust
commands::render_manager_bind_cfg
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
make rust-test
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/commands.rs src-tauri/src/file_tx.rs
git commit -m "feat(binds): 添加配置写入备份基础"
```

## 阶段 7：前端类型与页面骨架

### 任务 12：抽取前端类型与 Tauri 封装

**文件：**
- 创建：`src/types.ts`
- 创建：`src/lib/tauri.ts`
- 修改：`src/App.tsx`

- [ ] **步骤 1：创建类型文件**

在 `src/types.ts` 写入：

```typescript
export type LauncherState = "ready" | "downloading" | "running";

export type ClientHealth =
  | "ok"
  | "missing_executable"
  | "missing_storage_cfg"
  | "missing_data_dir";

export type ClientInstallation = {
  id: string;
  client_id: string;
  display_name: string;
  install_dir: string;
  executable_path: string;
  storage_cfg_path: string;
  data_dir: string;
  user_data_dir: string | null;
  version: string | null;
  is_default: boolean;
  health: ClientHealth;
};

export type BindRecord = {
  key: string;
  command: string;
  source_file: string;
  line: number;
  managed_by_manager: boolean;
  matched_workshop_id: string | null;
};

export type WorkshopBind = {
  id: string;
  category: string;
  title: string;
  command: string;
  description: string;
  command_variants: string[];
  variant_labels: string[];
  is_bindable: boolean;
};
```

- [ ] **步骤 2：创建 Tauri 封装**

在 `src/lib/tauri.ts` 写入：

```typescript
import { invoke } from "@tauri-apps/api/core";
import type { BindRecord, ClientInstallation, WorkshopBind } from "../types";

export function validateClientDir(path: string): Promise<ClientInstallation> {
  return invoke<ClientInstallation>("validate_client_dir", { path });
}

export function launchClient(path: string): Promise<void> {
  return invoke<void>("launch_client", { path });
}

export function analyzeCfgFile(path: string): Promise<BindRecord[]> {
  return invoke<BindRecord[]>("analyze_cfg_file", { path });
}

export function loadWorkshopBinds(url: string): Promise<WorkshopBind[]> {
  return invoke<WorkshopBind[]>("load_workshop_binds", { url });
}
```

- [ ] **步骤 3：修改 App 类型引用**

在 `src/App.tsx` 中删除本地 `LauncherState` 定义，改为：

```typescript
import type { LauncherState } from "./types";
```

保留当前 `VersionInfo` 和 `DownloadProgress`，直到更新面板替换 mock。

- [ ] **步骤 4：运行类型检查**

运行：

```bash
make check-lint
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src/types.ts src/lib/tauri.ts src/App.tsx
git commit -m "feat(frontend): 添加前端领域类型和 IPC 封装"
```

### 任务 13：拆分 TitleBar 与 LaunchPanel

**文件：**
- 创建：`src/components/layout/TitleBar.tsx`
- 创建：`src/components/launch/LaunchPanel.tsx`
- 修改：`src/App.tsx`

- [ ] **步骤 1：创建 TitleBar**

把 `App.tsx` 中的 `TitleBar` 组件移动到 `src/components/layout/TitleBar.tsx`：

```tsx
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, X } from "lucide-react";

const appWindow = getCurrentWindow();

export function TitleBar() {
  const minimize = () => {
    void appWindow.minimize();
  };

  const close = () => {
    void appWindow.close();
  };

  return (
    <div
      data-tauri-drag-region
      className="absolute left-0 top-0 z-50 flex h-11 w-full items-center justify-between border-b border-white/8 bg-black/20 px-4 backdrop-blur-md"
    >
      <div data-tauri-drag-region className="flex items-center gap-3 text-[11px] font-semibold uppercase tracking-[0.38em] text-cyan-100/70">
        <div className="h-2 w-2 rounded-full bg-cyan-300 shadow-[0_0_18px_rgba(65,242,255,0.95)]" />
        DDNet Manager
      </div>
      <div className="flex h-full items-center">
        <button aria-label="Minimize window" onClick={minimize} className="grid h-11 w-12 place-items-center text-slate-300/80 transition hover:bg-white/8 hover:text-cyan-100">
          <Minus size={16} strokeWidth={1.8} />
        </button>
        <button aria-label="Close window" onClick={close} className="grid h-11 w-12 place-items-center text-slate-300/80 transition hover:bg-red-500/90 hover:text-white">
          <X size={16} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
```

- [ ] **步骤 2：创建 LaunchPanel**

把启动按钮相关展示抽到 `src/components/launch/LaunchPanel.tsx`，先传入现有 props，不改变行为：

```tsx
import type { ReactNode } from "react";
import { Activity, Download, Play } from "lucide-react";
import { motion, useAnimationControls } from "framer-motion";
import type { LauncherState } from "../../types";

export type DownloadProgressView = {
  progress: number;
  downloaded_mb: number;
  total_mb: number;
  speed_mb_s: number;
  status: string;
};

export function LaunchButton(props: {
  state: LauncherState;
  progress: DownloadProgressView;
  hasPendingUpdate: boolean;
  onClick: () => Promise<void>;
}) {
  const controls = useAnimationControls();
  const click = async () => {
    await controls.start({
      x: [0, -3, 4, -2, 2, 0],
      scale: [1, 0.985, 1.012, 0.996, 1],
      transition: { duration: 0.24, ease: "easeOut" },
    });
    await props.onClick();
  };

  const title = props.state === "running" ? "游戏运行中" : props.state === "downloading" ? "部署中" : "开始游戏";
  const subtitle =
    props.state === "running"
      ? "DDNet client process handoff active"
      : props.state === "downloading"
        ? `${props.progress.status} · ${props.progress.speed_mb_s.toFixed(1)} MB/s`
        : props.hasPendingUpdate
          ? "启动前将自动部署最新稳定资源"
          : "配置已验证，客户端准备就绪";

  return (
    <motion.button onClick={click} animate={controls} disabled={props.state === "downloading"} whileHover={props.state === "downloading" ? undefined : { y: -2 }} whileTap={props.state === "downloading" ? undefined : { scale: 0.992 }} className="group relative h-[92px] w-[390px] overflow-hidden border border-cyan-200/35 bg-cyan-300 text-left text-slate-950 shadow-[0_0_42px_rgba(65,242,255,0.28)] transition disabled:cursor-default disabled:border-cyan-200/18 disabled:bg-slate-800 disabled:text-cyan-100" style={{ clipPath: "polygon(24px 0, 100% 0, 100% calc(100% - 24px), calc(100% - 24px) 100%, 0 100%, 0 24px)" }}>
      <div className="absolute inset-0 bg-[linear-gradient(120deg,rgba(255,255,255,0.55),rgba(255,255,255,0)_28%,rgba(0,0,0,0.18)_72%,rgba(0,0,0,0.42))]" />
      <div className="relative z-10 flex h-full items-center justify-between px-8">
        <div>
          <div className="flex items-center gap-3">
            {props.state === "running" ? <Activity size={25} /> : props.state === "downloading" ? <Download size={25} /> : <Play size={25} fill="currentColor" />}
            <span className="text-3xl font-black uppercase tracking-[0.08em]">{title}</span>
          </div>
          <div className="mt-2 text-[11px] font-bold uppercase tracking-[0.24em] opacity-70">{subtitle}</div>
        </div>
      </div>
    </motion.button>
  );
}
```

- [ ] **步骤 3：修改 App 引用**

在 `src/App.tsx` 删除原 `TitleBar` 和 `LaunchButton`，添加：

```typescript
import { TitleBar } from "./components/layout/TitleBar";
import { LaunchButton } from "./components/launch/LaunchPanel";
```

删除不再使用的 `getCurrentWindow`、`Minus`、`X`、`Play`、`Download` 导入。

- [ ] **步骤 4：运行类型检查**

运行：

```bash
make check-lint
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src/App.tsx src/components/layout/TitleBar.tsx src/components/launch/LaunchPanel.tsx
git commit -m "refactor(frontend): 拆分标题栏和启动按钮"
```

### 任务 14：添加 Manager 导航与页面占位

**文件：**
- 创建：`src/components/clients/ClientManager.tsx`
- 创建：`src/components/update/UpdatePanel.tsx`
- 创建：`src/components/resources/ResourcePanel.tsx`
- 创建：`src/components/binds/BindsPanel.tsx`
- 修改：`src/App.tsx`

- [ ] **步骤 1：创建页面占位组件**

创建 `src/components/clients/ClientManager.tsx`：

```tsx
export function ClientManager() {
  return (
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Clients</div>
      <h2 className="mt-3 text-2xl font-black">本地客户端</h2>
      <p className="mt-2 text-sm text-slate-300/80">扫描、手动添加并管理 QmClient / DDNet 客户端。</p>
    </section>
  );
}
```

创建 `src/components/update/UpdatePanel.tsx`：

```tsx
export function UpdatePanel() {
  return (
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Update</div>
      <h2 className="mt-3 text-2xl font-black">Manifest 更新</h2>
      <p className="mt-2 text-sm text-slate-300/80">读取自维护 manifest，通过 GitHub asset 下载更新。</p>
    </section>
  );
}
```

创建 `src/components/resources/ResourcePanel.tsx`：

```tsx
export function ResourcePanel() {
  return (
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Resources</div>
      <h2 className="mt-3 text-2xl font-black">资源位置</h2>
      <p className="mt-2 text-sm text-slate-300/80">区分安装目录、data 目录和用户配置目录。</p>
    </section>
  );
}
```

创建 `src/components/binds/BindsPanel.tsx`：

```tsx
export function BindsPanel() {
  return (
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Binds</div>
      <h2 className="mt-3 text-2xl font-black">Binds 管理</h2>
      <p className="mt-2 text-sm text-slate-300/80">分析本地 cfg，接入 Workshop，并通过安全事务写入。</p>
    </section>
  );
}
```

- [ ] **步骤 2：在 App 中添加导航状态**

在 `src/App.tsx` 添加类型：

```typescript
type AppView = "launch" | "clients" | "update" | "resources" | "binds";
```

添加状态：

```typescript
const [activeView, setActiveView] = useState<AppView>("launch");
```

添加渲染函数：

```tsx
const renderActiveView = () => {
  switch (activeView) {
    case "clients":
      return <ClientManager />;
    case "update":
      return <UpdatePanel />;
    case "resources":
      return <ResourcePanel />;
    case "binds":
      return <BindsPanel />;
    case "launch":
      return null;
  }
};
```

- [ ] **步骤 3：添加侧边导航**

在主布局中加入按钮组：

```tsx
const navItems: Array<{ id: AppView; label: string }> = [
  { id: "launch", label: "启动" },
  { id: "clients", label: "客户端" },
  { id: "update", label: "更新" },
  { id: "resources", label: "资源" },
  { id: "binds", label: "Binds" },
];
```

在右侧面板或主区域添加：

```tsx
<nav className="relative z-10 mt-4 grid grid-cols-5 gap-2">
  {navItems.map((item) => (
    <button
      key={item.id}
      onClick={() => setActiveView(item.id)}
      className={`border px-3 py-2 text-xs font-black uppercase tracking-[0.18em] transition ${
        activeView === item.id ? "border-cyan-200 bg-cyan-200 text-slate-950" : "border-white/10 bg-white/5 text-slate-300 hover:border-cyan-200/40"
      }`}
    >
      {item.label}
    </button>
  ))}
</nav>
```

在合适位置显示 `{renderActiveView()}`。

- [ ] **步骤 4：运行类型检查**

运行：

```bash
make check-lint
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src/App.tsx src/components/clients/ClientManager.tsx src/components/update/UpdatePanel.tsx src/components/resources/ResourcePanel.tsx src/components/binds/BindsPanel.tsx
git commit -m "feat(frontend): 添加 Manager 页面导航骨架"
```

## 阶段 8：前后端集成

### 任务 15：客户端管理页接入 validate_client_dir

**文件：**
- 修改：`src/components/clients/ClientManager.tsx`
- 修改：`src/lib/tauri.ts`
- 修改：`src/types.ts`

- [ ] **步骤 1：添加路径输入和状态**

在 `ClientManager.tsx` 中导入：

```typescript
import { useState } from "react";
import { validateClientDir } from "../../lib/tauri";
import type { ClientInstallation } from "../../types";
```

实现输入和按钮：

```tsx
const [path, setPath] = useState("");
const [client, setClient] = useState<ClientInstallation | null>(null);
const [error, setError] = useState<string | null>(null);

const validate = async () => {
  setError(null);
  try {
    setClient(await validateClientDir(path));
  } catch (err) {
    setError(err instanceof Error ? err.message : String(err));
  }
};
```

渲染：

```tsx
<input value={path} onChange={(event) => setPath(event.target.value)} className="mt-4 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100" placeholder="C:/Games/QmClient" />
<button onClick={validate} className="mt-3 border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-slate-950">验证目录</button>
{client && <pre className="mt-4 overflow-auto bg-black/30 p-3 text-xs text-cyan-100">{JSON.stringify(client, null, 2)}</pre>}
{error && <div className="mt-4 text-sm text-red-300">{error}</div>}
```

- [ ] **步骤 2：运行类型检查**

运行：

```bash
make check-lint
```

预期：PASS。

- [ ] **步骤 3：手动验证**

运行：

```bash
make tauri-dev
```

手动输入一个包含 `DDNet.exe`、`storage.cfg`、`data/` 的目录，预期页面显示 `health: ok`。如果无法启动桌面环境，记录 gap，不得声称手动验证完成。

- [ ] **步骤 4：Commit**

```bash
git add src/components/clients/ClientManager.tsx src/lib/tauri.ts src/types.ts
git commit -m "feat(frontend): 接入客户端目录验证"
```

### 任务 16：Binds 页接入 cfg 分析和 Workshop 数据

**文件：**
- 修改：`src/components/binds/BindsPanel.tsx`
- 修改：`src/lib/tauri.ts`
- 修改：`src/types.ts`

- [ ] **步骤 1：添加 cfg 分析 UI**

在 `BindsPanel.tsx` 导入：

```typescript
import { useState } from "react";
import { analyzeCfgFile, loadWorkshopBinds } from "../../lib/tauri";
import type { BindRecord, WorkshopBind } from "../../types";
```

添加状态：

```tsx
const [cfgPath, setCfgPath] = useState("");
const [records, setRecords] = useState<BindRecord[]>([]);
const [workshop, setWorkshop] = useState<WorkshopBind[]>([]);
const [error, setError] = useState<string | null>(null);
```

添加动作：

```tsx
const analyze = async () => {
  setError(null);
  try {
    setRecords(await analyzeCfgFile(cfgPath));
  } catch (err) {
    setError(err instanceof Error ? err.message : String(err));
  }
};

const loadWorkshop = async () => {
  setError(null);
  try {
    setWorkshop(await loadWorkshopBinds("https://ddrace.cn/data/binds.json"));
  } catch (err) {
    setError(err instanceof Error ? err.message : String(err));
  }
};
```

渲染输入、按钮和列表：

```tsx
<input value={cfgPath} onChange={(event) => setCfgPath(event.target.value)} className="mt-4 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100" placeholder="C:/Users/User/AppData/Roaming/DDNet/settings_ddnet.cfg" />
<div className="mt-3 flex gap-2">
  <button onClick={analyze} className="border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-slate-950">分析 cfg</button>
  <button onClick={loadWorkshop} className="border border-white/10 bg-white/5 px-4 py-2 text-sm font-black text-slate-100">加载 Workshop</button>
</div>
<div className="mt-4 grid grid-cols-2 gap-4">
  <pre className="max-h-64 overflow-auto bg-black/30 p-3 text-xs text-cyan-100">{JSON.stringify(records, null, 2)}</pre>
  <pre className="max-h-64 overflow-auto bg-black/30 p-3 text-xs text-amber-100">{JSON.stringify(workshop.slice(0, 5), null, 2)}</pre>
</div>
{error && <div className="mt-4 text-sm text-red-300">{error}</div>}
```

- [ ] **步骤 2：运行类型检查**

运行：

```bash
make check-lint
```

预期：PASS。

- [ ] **步骤 3：手动验证**

运行：

```bash
make tauri-dev
```

输入一个真实 cfg 文件路径，预期展示 bind 记录；点击加载 Workshop，预期展示线上前 5 条 Workshop Bind。若网络不可达，记录 gap。

- [ ] **步骤 4：Commit**

```bash
git add src/components/binds/BindsPanel.tsx src/lib/tauri.ts src/types.ts
git commit -m "feat(frontend): 接入 Binds 分析与 Workshop 数据"
```

## 阶段 9：收口验证

### 任务 17：项目级验证与文档同步

**文件：**
- 按需修改：`AGENTS.md`
- 按需修改：`CLAUDE.md`
- 修改：`docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md`

- [ ] **步骤 1：更新文档中的当前状态**

在 PRD 顶部追加实施状态：

```markdown
implementation_status: in_progress
```

如果命令入口或架构文件发生变化，同步 `AGENTS.md` 和 `CLAUDE.md`，保持两份文件文本一致。

- [ ] **步骤 2：验证 AGENTS 与 CLAUDE 一致**

运行：

```bash
powershell -NoProfile -Command "if ((Get-FileHash AGENTS.md).Hash -ne (Get-FileHash CLAUDE.md).Hash) { exit 1 }"
```

预期：exit 0。

- [ ] **步骤 3：运行完整检查**

运行：

```bash
make check-lint
```

预期：PASS。

- [ ] **步骤 4：运行前端构建**

运行：

```bash
make build
```

预期：PASS，输出 `dist/index.html` 和 assets。

- [ ] **步骤 5：提交收口**

```bash
git add docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md
git commit -m "docs(plan): 同步 DDNet Manager MVP 实施状态"
```

若本任务实际修改了 `AGENTS.md` 或 `CLAUDE.md`，提交命令改为：

```bash
git add AGENTS.md CLAUDE.md docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md
git commit -m "docs(agent): 同步 Manager 代理规则"
```

## 规格覆盖自检

- 客户端发现：任务 3、4、15 覆盖。
- manifest 更新：任务 5、6、16 部分覆盖；真实安装事务后续可拆单独计划继续。
- 网络加速：任务 5 预留 proxy_base_url；完整设置 UI 后续拆单独计划。
- 资源位置管理：任务 3 的 user_data_dir 和后续 ResourcePanel 骨架覆盖基础展示；完整目录打开后续拆单独计划。
- Binds 管理：任务 7、8、9、10、11、16 覆盖。
- 运行中写入保护：任务 4 提供进程基础，任务 11 建立写入事务基础；完整进程枚举阻断可在实现时扩展为独立任务。
- Workshop 接入：任务 9、16 覆盖。
- Everything 加速：本计划仅保留为后续能力，不进入第一轮实现任务。

## 验证命令总表

```bash
make rust-test
make rust-check
make check
make check-lint
make build
make tauri-dev
```

`make tauri-dev` 需要桌面环境和 WebView2。若无法运行，必须在交付说明中记录为验证 gap。
