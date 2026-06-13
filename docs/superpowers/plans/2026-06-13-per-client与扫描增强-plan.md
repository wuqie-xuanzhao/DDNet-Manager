---
type: forward-plan
date: 2026-06-13
status: active
scope:
  - per-client 设置（架构主线）
  - 多信号客户端识别
  - Everything 安装路径探测增强
  - 客户端更新位置下沉
  - 主界面占位符
  - 真实端到端验收
related:
  - file: ../explore/2026-06-13-后端验收链路打通与设置页对齐.md
    relation: 本计划是其验收链路打通后的体验对齐延续
  - file: 2026-06-07-network-route-selection-plan.md
    relation: 网络路由已重构为本地代理（本轮 8eb4106），该计划已落地
---

## 一、Context（为什么做这件事）

用户判断：后端能力验收已基本打通，剩余核心是**产品体验对齐**——

1. **每个客户端应能独立配置**（路径、下载位置、更新渠道、自动更新），当前是全局设置一刀切。
2. **客户端更新操作应在客户端卡片内**，而非藏在下载页（用户原话：「客户端更新为啥放到下载里面，应该放到客户端里面」）。
3. **扫描识别不应依赖文件夹名**——文件夹名不含特征时应仍能识别（靠文件签名/版本信息）。
4. **Everything 加速未生效**——es.exe 只认两个固定路径，装别处就不工作。

### 本轮已完成（4 个提交，2026-06-13）

| 提交 | 内容 | 净变更 |
|---|---|---|
| `8eb4106` | 网络路由改为本地代理隧道（reqwest proxy，支持 `http://127.0.0.1:7890`） | +225/-755 |
| `2ea7929` | 删除用户面向的 manifest 源（catalog 5 客户端全覆盖：QmClient/TaterClient/BestClient→GitHub，Cactus→官网，DDNet→官方下载） | +12/-106 |
| `bdd05f5` | 修复 C3 泛型逗号误判（awk 加 `<>` depth 计数）+ C4 icon unwrap（改 if let） | +12/-4 |
| `95a9655` | 浏览按钮（系统文件对话框选 DDNet.exe）+ 扫描空结果提示 + 检查更新按钮样式统一 | +30/-2 |

门禁：`PASS 15 / WARN 7 / FAIL 0`。剩余 7 个 WARN = C1 行数（5 文件，存量）+ C2 main()（111 行）+ A4 cargo audit（atk/gdk/gtk 系列 Linux unmaintained，**Windows 不受影响**，上游 Tauri 未迁移，不可直接修）。

## 二、per-client 设置（核心架构主线）

### 现状校准

- 设置是**全局 `AppSettings` 单对象**；`ClientInstallation` 无 per-client 偏好字段。
- 下载缓存统一在 `app_cache_dir/downloads`（commands.rs:393）；`auto_check_updates` / 渠道是全局或面板 state。
- 更新操作集中在下载页 `UpdatePanel`（客户端选择 + 检查更新 + 下载 + 安装）。

### 字段设计（`ClientInstallation` 新增）

| 字段 | 类型 | 语义 | 复杂度 |
|---|---|---|---|
| `auto_update` | `bool` | 该客户端是否自动检查更新 | 简单 |
| `update_channel` | `String` | 该客户端更新渠道（`stable`/`nightly`） | 简单 |
| `download_cache_dir` | `Option<String>` | 该客户端下载缓存目录（`None` = 用全局默认） | **复杂**（涉及 staging/rollback 事务） |

### 第一阶段：`auto_update` + `update_channel` + 客户端卡片内置更新（~1 天）

> 不碰下载事务目录语义，风险可控。

**后端**：
- `models.rs` `ClientInstallation` 加 `auto_update`（`#[serde(default)]`）+ `update_channel`（`#[serde(default = "stable")]`）。
- `registry.rs` SQLite 迁移：`ALTER TABLE client_installations ADD COLUMN auto_update INTEGER DEFAULT 0; ADD COLUMN update_channel TEXT DEFAULT 'stable';`（向后兼容旧记录）。
- `client_scan.rs` `validate_client_dir` 构造新字段默认值。
- 测试 fixture 更新（`test/models.rs`、`test/registry.rs`、`test/client_scan.rs`）。

**前端**：
- `types.ts` `ClientInstallation` 加字段。
- `ClientManager.tsx` 客户端卡片：显示 + 编辑 `auto_update`（toggle）/ `update_channel`（下拉）。
- `ClientManager.tsx` 卡片内置「检查更新 → 下载 → 安装」入口（从 `UpdatePanel` 下沉的更新逻辑）。
- `useAutoUpdate.ts` 改为读 per-client `auto_update`（而非全局 `auto_check_updates`）；requestKey 含 per-client 字段。
- `UpdatePanel.tsx` 收敛：客户端选择 + 检查更新 UI 移除，保留**全局下载历史 + 恢复 + 网络路由**。

### 第二阶段：`download_cache_dir`（~1 天，单独做）

> 改下载事务目录语义，多客户端并发安装需隔离。

- `commands.rs` `prepare_update_download_job`：`downloads_dir` 改为 per-client `download_cache_dir`（fallback 全局 `app_cache_dir/downloads`）。
- `download.rs` staging/rollback 目录跟着 per-client 化（避免两个客户端安装事务互相干扰）。
- `ClientManager.tsx` 卡片：下载位置编辑（浏览目录，复用 openDialog）。
- 测试：多客户端并发下载事务隔离；旧全局缓存的兼容/迁移。

## 三、多信号客户端识别（不依赖文件夹名）

### 现状

`infer_client_identity`（`client_scan.rs:555`）靠**路径子串**（`match_catalog_entry` 的 aliases）+ `steamapps` 路径判断。文件夹名不含 alias 时 → `third_party`。用户原话：「识别不能依赖文件夹的名称」。

### 改造：多信号融合

按置信度优先级：
1. **PE 版本信息**（Windows）：读 `DDNet.exe` 的 `VS_VERSION_INFO` resource（`ProductName` / `CompanyName` / `FileDescription`），匹配 catalog 的 `display_name` / upstream。纯 Rust 解析用 `pelite`（无 unsafe）或 `windows-sys`。
2. **`data/` 目录特征文件**：logo、字体、配置文件名特征（如 QmClient 特有资源）。
3. **文件名 + 路径别名**（降为辅助信号，不是唯一）。
4. **置信度升级**：`ClientConfidence`（Verified/Compatible/Partial/Unsupported）融合多信号后评定。

### 实现

- 新建 `pe_version.rs`（PE version resource 解析）。
- `infer_client_identity` 改为信号融合：PE 版本 > 路径别名 > 文件名 > `third_party`。
- `client_catalog.rs` 加版本特征匹配规则（`ProductName` 含 "QmClient" 等）。
- 测试：各客户端 PE fixture（脱敏样本，覆盖带/不带 version resource 两种情况）。

## 四、Everything 安装路径探测增强

### 现状

`everything_executable_candidates`（`client_scan.rs:414`）只认 `C:/Program Files/[Everything/]es.exe` 两个固定路径。Everything 装在别处（如 D 盘、用户目录）就不工作；且默认 `use_everything=false`，用户不知道要开。

### 改造

- 加注册表查询：`HKLM\SOFTWARE\voidtools\Everything` 的 `InstallLocation`（`reg query`，仿 `set_autostart_registry` commands.rs:222 模式）+ WOW6432Node fallback。
- `everything_executable_candidates` = 固定路径 ∪ 注册表路径，全部 `is_file()` 过滤。
- 工具页「使用 Everything 加速扫描」改为**默认开启探测**（找到 es.exe 才用，找不到静默回退普通扫描，不报错）。
- `#[cfg(target_os = "windows")]` 守卫注册表查询。

## 五、客户端更新位置下沉（与 per-client 第一阶段绑定）

用户要求：客户端更新应在客户端卡片，不在下载页。

- 第一阶段下沉时解决（见 per-client 第一阶段前端）。
- `UpdatePanel` 保留职责：全局下载历史 + 恢复任务 + 网络路由（这些是全局的，不属于具体客户端）。
- 主界面「获取游戏/开始游戏」按钮可考虑接 per-client 更新状态（体验打磨，优先级低）。

## 六、主界面占位符（D1，视觉收口）

用户确认：主界面大屏 UI 视觉保持（米哈游风格复刻），仅占位符待填。

- 背景图：外观设置已有 `customBgs` 框架（每客户端 image/video），需素材填充默认背景。
- 公告：`NewsCard` 占位，需公告数据源（当前无后端）。
- **非后端工作**，需素材/数据源，优先级最低。

## 七、真实端到端验收（用户补验，自动化覆盖不到）

1. **本地代理 `127.0.0.1:7890` 全链路**：检查更新 → 下载 → sha256 校验 → 安装（含 GitHub 不可达时代理兜底）。
2. **浏览按钮手动添加客户端**：选 DDNet.exe → 自动取目录 → 验证保存。
3. **per-client 设置生效**（第一阶段后）：每个客户端独立 auto_update/channel。
4. **多客户端识别**（多信号后）：文件夹名不含特征仍能识别为 QmClient/TaterClient 等。
5. **Windows 桌面端到端**（next-mvp-spec 验收矩阵）：扫描 → 设默认 → 检查更新 → 下载 → 安装 → 启动 → 运行中安装阻断 → 错误 sha256 拒绝 → 路径穿越 zip 拒绝。
6. macOS/Linux 安装事务（spec 自述缺口，本次未涉及）。

## 八、优先级 + 依赖关系

| 序 | 项目 | 工作量 | 依赖 | 建议时机 |
|---|---|---|---|---|
| 1 | per-client 第一阶段（auto_update + channel + 卡片内置更新） | ~1 天 | 无 | **新会话，主线** |
| 2 | 多信号客户端识别（PE 版本 + 信号融合） | ~1-1.5 天 | 无 | 独立，可并行 |
| 3 | Everything 注册表探测增强 | ~0.5 天 | 无 | 独立小项 |
| 4 | per-client 第二阶段（download_cache_dir + 事务隔离） | ~1 天 | 依赖 #1 | #1 之后 |
| 5 | 主界面占位符填充 | 需素材 | 无 | 最低 |

建议**新会话逐项做**，每项独立提交。per-client 第一阶段是主线，应专注一轮。

## 九、风险

- **per-client `download_cache_dir`**：改下载事务目录语义，多客户端并发安装需隔离测试（staging/rollback 目录 per-client 化）。这是第二阶段单列的原因。
- **PE 版本解析**：依赖 exe 带 `VS_VERSION_INFO` resource（部分 fork 可能不带 → 回退路径别名/文件名，不崩溃）。
- **Everything 注册表查询**：Windows-only，需 `cfg(target_os = "windows")`；注册表结构变化时 fallback 固定路径。
- **客户端卡片内置更新下沉**：大幅改 `UpdatePanel` / `ClientManager` 结构，注意保留下载历史/恢复 UI（全局，不下沉）。
- **SQLite 迁移**：`ALTER TABLE ADD COLUMN` 对旧记录用 DEFAULT 兼容；但若字段从 NOT NULL 无 DEFAULT，旧记录读取失败——务必带 DEFAULT。

## 十、关键文件（按项目）

- **per-client**：`models.rs`（ClientInstallation）、`registry.rs`（迁移 + load/upsert）、`client_scan.rs`（validate_client_dir 构造）、`commands.rs`（下载 dir）、`types.ts`、`ClientManager.tsx`、`UpdatePanel.tsx`、`useAutoUpdate.ts`。
- **多信号识别**：`client_scan.rs`（infer_client_identity）、新建 `pe_version.rs`、`client_catalog.rs`（版本特征规则）。
- **Everything**：`client_scan.rs`（everything_executable_candidates + 新增注册表查询）。

## 十一、可复用的已有能力（不要重写）

- `@tauri-apps/plugin-dialog` openDialog（SettingsDialog 外观自定义背景已用，ClientManager 浏览按钮已用）——per-client 下载位置浏览复用。
- `ClientConfidence`（Verified/Compatible/Partial/Unsupported）——多信号识别后直接用，无需新枚举。
- `ClientInstallation.compatibility`——per-client 兼容性诊断已就位。
- `build_routed_client`（network_route.rs）——本地代理隧道已统一注入，per-client 更新下载自动走代理。
- catalog 5 客户端 `UpdateSourceDescriptor`（GitHub/Website/DdnetOfficial）——per-client 渠道选择直接复用。

## 十二、验证方式

- 每项完成跑 `make check-lint`（FAIL=0；新引入 WARN 须说明）。
- per-client：多客户端 fixture 测试（auto_update/channel 独立读写）+ 迁移兼容（旧 DB 升级）。
- 多信号：PE fixture（带/不带 version resource）+ 路径别名回退 + 文件名回退。
- Everything：注册表 mock（CI 无 Everything）+ 固定路径 fallback。
- 最终：用户在真实 Windows 环境跑第七节验收清单。
