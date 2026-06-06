# DDNet Manager 后端强化规格：客户端管理与更新下载优先

status: draft
date: 2026-06-06
scope: backend-strengthening-ab-first
related:
- `docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md`
- `docs/superpowers/explore/2026-06-06-后端强化探索.md`

## 背景

当前 DDNet Manager 已有 Tauri v2 后端骨架、手动客户端目录验证、manifest 元数据读取、cfg 解析和 Workshop JSON 拉取能力。但启动器核心闭环尚未完成：应用不能自动发现本机多个 DDNet/QmClient 客户端，不能保存默认客户端，也不能真实下载、校验、安装和回滚更新包。

本规格将后端强化优先级调整为 A+B：

- A：客户端管理核心。
- B：真实更新下载链路。
- C：Binds 安全写入后置，只预留接口，不在本阶段实现写入。

## 目标

- 建立本地客户端注册表，支持自动扫描、手动添加、默认客户端和路径健康状态。
- 支持 QmClient-first，同时为 DDNet Vanilla、QmClient Nightly 和第三方客户端预留分类。
- 用 manifest 作为更新权威源，GitHub 或镜像只作为资产下载位置。
- 实现真实下载任务：进度事件、sha256 校验、临时文件、解压、安装事务和失败恢复。
- 明确网络加速边界：MVP 优先代理和镜像 URL，不默认修改系统 hosts。
- 为后续 Binds 写入安全预留“客户端运行状态”和“用户 cfg 路径”能力。

## 非目标

- 不实现 Binds 写入、diff 预览和回滚应用。
- 不实现 hosts 自动修改。
- 不实现完整 Workshop 客户端能力。
- 不扫描或管理 QmClient 开发仓库的 Release 目录作为用户安装源。
- 不实现跨平台完整扫描策略；本阶段 Windows 优先。

## 阶段 A：客户端管理核心

### 能力范围

后端新增客户端注册表，负责保存本机识别到的客户端安装实例。注册表至少保存：

- 安装记录 ID。
- 客户端类型：`qmclient`、`ddnet_vanilla`、`qmclient_nightly`、`third_party`。
- 展示名称。
- 安装目录。
- 可执行文件路径。
- `storage.cfg` 路径。
- 安装包内 `data` 目录。
- 用户数据目录。
- 版本号或未知状态。
- 默认客户端标记。
- 健康状态。
- 最后扫描时间。

### 扫描策略

默认扫描应覆盖：

- 已保存的历史路径。
- 常见游戏安装目录，如 `C:/Games`、`D:/Games`、`C:/Program Files`、`C:/Program Files (x86)`。
- Windows 用户数据目录，如 `%APPDATA%/DDNet`、`%APPDATA%/QmClient`。
- Steam 常见路径中的 DDNet 目录。

深度扫描应由用户手动触发，不能默认全盘扫。Everything 是可选 provider：

- 如果检测到 `es.exe` 或 Everything SDK，则用它快速查找 `DDNet.exe`、`storage.cfg`、`settings_ddnet.cfg`。
- 未安装 Everything 时必须退化为普通扫描，不影响基本功能。
- 扫描结果先作为候选，只有通过结构验证后才进入注册表。

### 客户端识别

识别规则按保守优先：

- `DDNet.exe`、`storage.cfg`、`data/` 同时存在，健康状态为 `ok`。
- 缺少任一关键结构时仍可返回候选，但健康状态必须标明缺失项。
- QmClient-first，若无法可靠识别具体客户端类型，则归类为 `third_party`，不强行写死为 QmClient。
- 版本识别失败时返回 `null`，不得伪造版本。

### Tauri Commands

建议新增或替换为以下 command：

- `scan_client_installations(options)`：扫描本机候选客户端并返回列表。
- `validate_client_dir(path)`：保留现有能力，但返回更完整分类结果。
- `upsert_client_installation(client)`：保存或更新客户端记录。
- `remove_client_installation(id)`：从注册表移除记录，不删除本地文件。
- `set_default_client(id)`：设置默认启动客户端。
- `list_client_installations()`：读取注册表。
- `get_default_client()`：读取默认客户端。

### 持久化

本阶段推荐使用 SQLite，而不是只用 JSON 文件。原因是后续要保存扫描缓存、更新历史、下载任务、回滚点和 Bind 安装历史，SQLite 更适合演进。

最小表：

- `client_installations`
- `app_settings`
- `scan_history`

迁移策略：

- 初期可用轻量 schema 初始化函数。
- 后续如果 schema 增长，再引入正式 migration 层。

## 阶段 B：真实更新下载链路

### Manifest 策略

manifest 是唯一更新权威源。GitHub release、GitHub asset 或镜像地址只负责提供下载文件。

manifest 至少包含：

- `schema_version`
- `client_id`
- `channel`
- `version`
- `platform`
- `asset_url`
- `sha256`
- `size`
- `release_notes`

后端需要实现：

- 按默认客户端的 `client_id` 和用户选择的 `channel` 过滤 manifest。
- 按当前平台选择 asset。
- 比较本地版本和远端版本。
- 将 manifest 拉取、解析、选择结果拆开，避免 UI 自己理解 manifest。

### 网络与镜像

当前 `proxy_base_url + url` 的拼接方式需要重新设计。推荐分为三类：

- `direct`：直接请求 manifest 和 asset。
- `proxy_prefix`：将原始 URL 编码后交给明确配置的代理服务。
- `mirror_template`：用模板替换 GitHub asset 地址，例如把 GitHub release asset 映射到用户配置的镜像域名。

安全要求：

- 默认只允许 HTTPS。
- 禁止 localhost、私网 IP、link-local 地址。
- manifest host 和 asset host 仍应有 allowlist，但镜像 host 必须由设置显式启用。
- 禁止自动修改 hosts。

### 下载任务

下载系统需要从 mock `start_download` 替换为真实 job。

最小 command：

- `check_client_update(client_id, channel)`：返回当前版本、最新版本、asset、是否需要更新。
- `start_update_download(request)`：创建下载任务并开始下载。
- `cancel_download(job_id)`：取消下载任务。
- `get_download_job(job_id)`：查询任务状态。
- `install_downloaded_update(job_id)`：校验并安装已下载包。

事件：

- `download-progress`
- `download-completed`
- `download-failed`
- `install-progress`
- `install-completed`
- `install-failed`

### 下载事务

下载必须写入 Manager 自有缓存目录，不能直接覆盖客户端目录。

事务流程：

1. 创建下载 job。
2. 下载到临时文件。
3. 校验字节数和 sha256。
4. 解压到 staging 目录。
5. 检查 staging 内客户端结构。
6. 创建当前安装目录回滚点。
7. 原子切换或安全覆盖。
8. 更新注册表版本与安装状态。
9. 清理临时文件。

失败恢复：

- 下载失败不影响当前客户端。
- 校验失败删除临时文件并记录错误。
- 安装失败保留回滚点。
- 用户可手动清理下载缓存。

### 包格式

第一阶段只支持 zip 包。后续如需要 7z 或自解压包，单独追加。

zip 安全要求：

- 拒绝路径穿越，如 `../`。
- 拒绝绝对路径。
- 拒绝解压到目标目录外。
- 限制解压总大小和文件数量。

## 阶段 C：Binds 安全写入后置

C 不进入本阶段实现，但 A+B 需要预留两个接口：

- 能通过注册表找到默认客户端的用户数据目录和候选 cfg 文件。
- 能通过进程模块判断目标客户端是否正在运行。

后续 C 阶段再实现：

- diff 预览。
- 写入前备份。
- Manager 专用 cfg。
- settings/autoexec 标记区块。
- 运行中禁止写 cfg。
- 回滚。

## 数据模型草案

### ClientInstallation

```json
{
  "id": "qmclient-0010748f07d5a3e0",
  "client_id": "qmclient",
  "display_name": "QmClient",
  "install_dir": "C:/Games/QmClient",
  "executable_path": "C:/Games/QmClient/DDNet.exe",
  "storage_cfg_path": "C:/Games/QmClient/storage.cfg",
  "data_dir": "C:/Games/QmClient/data",
  "user_data_dir": "C:/Users/User/AppData/Roaming/DDNet",
  "version": null,
  "is_default": true,
  "health": "ok",
  "last_scanned_at": "2026-06-06T12:00:00Z"
}
```

### DownloadJob

```json
{
  "id": "download-20260606-001",
  "client_installation_id": "qmclient-0010748f07d5a3e0",
  "client_id": "qmclient",
  "channel": "stable",
  "version": "2.62.5",
  "asset_url": "https://github.com/example/qmclient.zip",
  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "size": 89531134,
  "status": "downloading",
  "downloaded_bytes": 1048576,
  "cache_path": "C:/Users/User/AppData/Local/DDNet Manager/downloads/download-20260606-001.zip"
}
```

### InstallTransaction

```json
{
  "id": "install-20260606-001",
  "download_job_id": "download-20260606-001",
  "target_installation_id": "qmclient-0010748f07d5a3e0",
  "staging_dir": "C:/Users/User/AppData/Local/DDNet Manager/staging/install-20260606-001",
  "rollback_dir": "C:/Users/User/AppData/Local/DDNet Manager/rollback/install-20260606-001",
  "status": "prepared"
}
```

## 前端契约影响

前端应停止依赖 mock 的 `check_update`、`start_download` 和 `launch_game`。

首页启动按钮应使用：

- `get_default_client()`
- `launch_client(executable_path)`

更新页应使用：

- `check_client_update(client_id, channel)`
- `start_update_download(request)`
- 下载和安装事件。

全部游戏页应使用：

- `list_client_installations()`
- `scan_client_installations(options)`
- `set_default_client(id)`

## 验收标准

阶段 A 完成时：

- 启动应用后能列出已保存客户端。
- 能扫描常见路径并返回候选客户端。
- 能手动添加客户端目录。
- 能保存默认客户端。
- 能识别健康状态，不再把所有目录强行当作 QmClient。

阶段 B 完成时：

- 能从 manifest 获取默认客户端更新状态。
- 能下载真实 zip 包并显示真实进度。
- 能校验 sha256。
- 能解压到 staging。
- 能在安装失败时保留原客户端。
- UI 不再出现模拟的 `936MB` 更新包。

## 验证计划

自动验证：

```bash
make rust-test
make rust-check
make check-lint
```

手动验证：

- 使用真实 QmClient 目录验证扫描和启动。
- 使用测试 manifest 验证更新检查。
- 使用小型测试 zip 验证下载、校验、解压和安装事务。
- 断网或错误 sha256 时验证失败恢复。

## 风险

- Windows 文件占用会影响安装覆盖，安装事务必须检测目标进程是否运行。
- GitHub 网络不稳定，镜像和代理不能做成简单字符串拼接。
- Everything 只能作为加速 provider，不能作为硬依赖。
- zip 解压存在路径穿越风险，必须做路径归一化和目标目录边界校验。
- 版本识别可能不可靠，不能为了 UI 好看伪造版本。
