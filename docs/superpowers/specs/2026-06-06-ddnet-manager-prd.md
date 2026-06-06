# DDNet Manager PRD

status: draft
implementation_status: partial_implementation_verified
date: 2026-06-06
scope: QmClient-first MVP

implementation_note: 首版脚手架、基础 IPC、基础后端模块、前端页面骨架、cfg 深度解析、manifest 元数据读取和资源路径识别已通过自动门禁；PRD 级 MVP 尚未完全收口，仍需补齐真实下载、校验、安装、回滚事务和桌面手动联调。

## 背景

DDNet Manager 是面向 DDNet / QmClient 玩家的一体化启动器与配置管理器。它不是 QmClient 的构建系统，也不替代 QmClient 仓库的 CMake、发布和开发流程。它面向最终用户，负责发现本机客户端、下载更新、启动游戏、管理资源路径，并识别和修改用户实际使用的 Binds 配置。

第一版聚焦 QmClient，但底层模型必须预留多第三方客户端。未来可管理 DDNet 原版、TaterClient、QmClient 或其他兼容客户端。

## 产品目标

- 让用户无需手动查找目录即可发现本机已有 DDNet/QmClient 客户端。
- 让用户通过自维护 manifest 从 GitHub 下载和更新 QmClient。
- 让用户清楚区分客户端安装目录、用户数据目录和资源目录。
- 让用户在启动器中查看、分析、安装和回滚 Binds 配置。
- 让网络不可达问题通过代理或镜像优先解决，而不是默认修改系统 hosts。

## 非目标

- 不把开发仓库的 `QmClient_Release` 当作用户更新源。
- 不替代 QmClient 的 CMake 构建体系。
- 不默认修改系统 hosts。
- 不在游戏客户端运行时写入 cfg 文件。
- 不做完整的 Workshop 社区客户端，不实现评分、评论、账号体系或内容发布。

## 当前技术栈

- **桌面壳**：Tauri v2。
- **后端**：Rust 2021、tokio、reqwest、serde、serde_json。
- **前端**：React 19、TypeScript 6、Vite 8、Tailwind CSS v4、Framer Motion、lucide-react。
- **包管理与编排**：Bun、Cargo、Makefile。
- **目标平台**：Windows 优先。

## 后续需要补齐的技术能力

- SQLite：保存客户端注册表、扫描缓存、更新历史、Bind 安装历史和回滚记录。
- 下载事务：断点续传、sha256 校验、zip 解压、原子切换、失败恢复。
- 进程监控：检测 `DDNet.exe` / QmClient 运行状态，防止运行中写配置。
- cfg 解析器：解析 `bind`、`unbind`、`exec`、注释、重复绑定和跨文件依赖。
- 文件事务：写入前备份、差异预览、标记区块、回滚。
- 可选 Everything provider：检测 `es.exe` 或 Everything SDK，用索引加速本地扫描。
- 网络 provider：支持代理、镜像 URL、GitHub 可达性诊断。

## 核心用户故事

### 客户端发现

用户打开 DDNet Manager 后，应用自动扫描常见路径，识别本机已有客户端。若没有找到，用户可以手动选择客户端目录。用户也可以触发深度扫描。

验收标准：

- 能识别包含 `DDNet.exe`、`storage.cfg` 和 `data/` 的完整客户端目录。
- 能显示客户端类型、路径、版本状态和健康状态。
- 能将多个客户端实例保存到本地注册表。
- 能设置默认启动客户端。

### QmClient 更新

用户在 Manager 中看到当前 QmClient 版本和最新版本。Manager 读取自维护 manifest，下载 GitHub 托管的 zip 包，校验后安装或更新。

验收标准：

- manifest 是更新权威源，GitHub 只作为资产下载地址。
- manifest 至少包含 client_id、channel、version、platform、asset_url、sha256、size、release_notes。
- 支持 stable / preview 渠道。
- 下载支持代理或镜像 URL。
- 更新前保留回滚点。
- 更新失败不会破坏当前可用客户端。

### 网络加速

用户网络无法访问 GitHub 时，Manager 优先提供代理和镜像 URL 配置。Host 修改作为高级实验能力后置，不进入 MVP 默认流程。

验收标准：

- MVP 提供 GitHub 可达性诊断。
- MVP 支持配置代理或镜像 URL。
- MVP 不自动修改 hosts。
- 后续若支持 hosts，必须显式启用、请求管理员权限、写入可识别标记区块，并支持一键恢复。

### 资源位置管理

用户可以看到安装目录和用户数据目录。Manager 能解释 `storage.cfg` 的路径优先级，帮助用户打开 skins、maps、demos、screenshots、configs 等目录。

验收标准：

- 能区分客户端安装包内 `data/` 与用户数据目录。
- Windows 默认识别 `%APPDATA%/DDNet` 和 `%APPDATA%/QmClient` 相关目录。
- 用户可以手动添加或修正目录。
- 不把资源写入错误位置。

### Binds 管理

用户在 Manager 中查看当前实际使用的 Binds。Manager 扫描 C 盘用户数据目录中的 cfg 文件，解析 bind 指令、exec 链和按键冲突。用户可以从 Workshop 安装 Bind，Manager 负责备份、差异预览、写入和回滚。

验收标准：

- 默认扫描 `%APPDATA%/DDNet`、`%APPDATA%/QmClient` 及用户手动选择的 cfg 目录。
- 能解析 `bind <key> <command>`、`unbind <key>` 和 `exec <file>`。
- 能展示按键占用、来源文件、重复绑定和缺失 exec 目标。
- 客户端运行时禁止写 cfg，只允许只读分析。
- 写入前必须备份。
- 写入前必须展示差异预览。
- 支持一键回滚。

## Workshop 接入

Workshop 来自自有项目 `E:/Coding/Web/DDNet_Bind`，线上地址为 `https://ddrace.cn/bind`。MVP 不做完整 Workshop 客户端，只抓取或消费网页公开内容。

当前可消费数据：

- `public/data/binds.json`：Bind 条目、分类、命令、变体、描述。
- `public/data/cfg-groups.json`：cfg 文件组和下载 URL。
- `public/data/full-packages.json`：完整包。

MVP 策略：

- 优先消费线上公开静态 JSON。
- 若网页结构变化，Manager 通过 Workshop adapter 层隔离变化。
- 本地 cfg 识别与 Workshop 条目匹配时，优先使用 command 或 commandVariants 进行相似度匹配。
- 不做评分、评论、登录、收藏同步等社区能力。

## Binds 写入策略

采用安全混合模式。

默认行为：

- 生成 Manager 专用 cfg，例如 `ddnet_manager_binds.cfg`。
- 在用户配置中通过可识别标记区块接入该 cfg。
- 所有 Manager 写入内容必须带标记，便于后续更新和清理。

高级行为：

- 用户显式选择后，允许直接编辑 settings 或 autoexec。
- 直接编辑必须先展示差异。
- 直接编辑必须创建备份。
- 游戏客户端运行中禁止直接编辑。

运行中保护：

- 检测到目标客户端进程运行时，Manager 不允许写 cfg。
- 运行中只允许查看、分析和生成待应用计划。
- 用户关闭客户端后才能应用修改。

## 本地客户端扫描策略

采用混合扫描。

默认扫描：

- 常见安装路径。
- 常见用户数据路径。
- Manager 已记录的历史路径。

手动添加：

- 用户可选择客户端目录。
- Manager 验证目录是否包含完整客户端结构。

深度扫描：

- 用户手动触发。
- 支持排除目录。
- 扫描目标包括 `DDNet.exe`、`storage.cfg`、`data/` 和常见 cfg 文件。

Everything 加速：

- Everything 是可选 provider，不是硬依赖。
- 如果检测到 `es.exe` 或 Everything SDK，则使用索引快速查找候选文件。
- 未安装 Everything 时功能不降级，只是扫描更慢。

## 数据模型草案

### ClientInstallation

```json
{
  "id": "qmclient-local-1",
  "client_id": "qmclient",
  "display_name": "QmClient 2.62.5",
  "install_dir": "C:/Games/QmClient",
  "executable_path": "C:/Games/QmClient/DDNet.exe",
  "storage_cfg_path": "C:/Games/QmClient/storage.cfg",
  "data_dir": "C:/Games/QmClient/data",
  "user_data_dir": "C:/Users/User/AppData/Roaming/DDNet",
  "version": "2.62.5",
  "is_default": true,
  "health": "ok"
}
```

### UpdateManifest

```json
{
  "schema_version": 1,
  "clients": [
    {
      "client_id": "qmclient",
      "channel": "stable",
      "version": "2.62.5",
      "platform": "windows-x64",
      "asset_url": "https://github.com/wxj881027/QmClient/releases/download/v2.62.5/QmClient-2.62.5-win64.zip",
      "sha256": "hex",
      "size": 89531134,
      "release_notes": "https://github.com/wxj881027/QmClient/releases/tag/v2.62.5"
    }
  ]
}
```

### BindRecord

```json
{
  "key": "mouse3",
  "command": "echo Kill; kill",
  "source_file": "settings_ddnet.cfg",
  "line": 128,
  "managed_by_manager": false,
  "matched_workshop_id": "bind-防自杀-fb77af69c13c"
}
```

## 主要页面

- 首页 / 启动页：展示默认客户端、版本状态、更新状态、启动按钮。
- 客户端管理页：列出本地客户端，支持扫描、手动添加、设置默认、打开目录。
- 更新页：展示 manifest 最新版本、下载状态、代理/镜像设置、更新日志、回滚。
- 资源页：展示安装目录和用户数据目录，快速打开 skins、maps、demos、configs。
- Binds 页：展示本地 binds、Workshop 条目、冲突分析、安装、差异预览和回滚。
- 设置页：代理/镜像、扫描排除路径、Everything provider 状态、实验功能。

## 风险与约束

- DDNet/QmClient 客户端可能在运行时改写 cfg，Manager 必须避免运行中写入。
- cfg 语法和 exec 链可能复杂，MVP 解析器应先覆盖常见模式，再逐步扩展。
- GitHub 网络可达性在国内不稳定，代理/镜像必须是 MVP 能力。
- Everything 不能作为硬依赖。
- Workshop 线上结构可能变化，需要 adapter 隔离。
- Host 修改涉及系统权限和安全风险，不进入 MVP 默认流程。

## MVP 交付边界

MVP 完成时，用户应该可以：

- 安装并打开 DDNet Manager。
- 自动或手动添加 QmClient。
- 看到 QmClient 是否需要更新。
- 通过 manifest + GitHub asset 下载并安装更新。
- 启动默认 QmClient。
- 查看本机 cfg 中实际 bind 占用。
- 从 Workshop 安装 bind 到安全的 Manager 专用 cfg。
- 在写入前看到差异，并能回滚。

MVP 不要求：

- 多账号。
- 社区登录。
- Workshop 评分、收藏、评论。
- 自动发布 QmClient。
- 自动修改 hosts。
- 完整语义级 cfg 重构。
