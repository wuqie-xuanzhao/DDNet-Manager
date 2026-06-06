# 项目代理指南

此文件同时用于 `AGENTS.md` 和 `CLAUDE.md`。两份文件必须保持文本一致，避免不同 AI 编程代理读取到互相冲突的仓库规则。

## 项目定位

DDNet Manager 是面向 DDNet / QmClient 玩家的第三方游戏启动器与管理器。第一版以 QmClient 为主线，但架构需要预留多第三方客户端。

核心职责：

- 发现和注册本机 DDNet / QmClient 客户端。
- 通过自维护 manifest 管理 QmClient 下载、更新、校验和回滚。
- 区分客户端安装目录、用户数据目录、资源目录。
- 分析和安全写入 DDNet / QmClient Binds 配置。
- 消费 `https://ddrace.cn/bind` 的公开 Workshop 数据。

非目标：

- 不把 QmClient 开发仓库的 Release 目录当成用户更新源。
- 不替代 QmClient 的 CMake / 发布流程。
- 不默认修改系统 hosts。
- 不在游戏客户端运行时写 cfg 文件。
- 不实现完整 Workshop 社区客户端。

## 技术栈

- **前端**：React 19 + TypeScript 6 + Vite 8 + Tailwind CSS v4 + Framer Motion
- **后端**：Tauri v2 + Rust 2021 + tokio + reqwest + serde
- **包管理**：Bun（JS/TS）、Cargo（Rust）
- **命令编排**：Makefile
- **目标平台**：Windows 优先，窗口使用 `windows_subsystem = "windows"`

## 命令规则

仓库内的开发、检查、构建、预览、Tauri 启动命令统一通过 `make` 调用。不要在文档或脚本说明中直接推荐底层包管理器或编译器命令作为日常入口；如需新增命令，先添加到 `Makefile`，再在文档中引用对应的 `make` 目标。

### 常用命令

```bash
make install         # 安装 JS/TS 依赖
make dev             # 启动 Vite 前端开发服务器
make tauri-dev       # 启动完整 Tauri 桌面应用
make check           # 轻量检查：TypeScript build + cargo check
make check-lint      # 默认验收门禁：fmt + clippy + test + TS + 结构扫描
make check-lint-fix  # 运行门禁并自动执行 cargo fmt
make fmt             # Rust 格式化
make test            # 默认测试入口
make rust-test       # Rust 测试
make rust-check      # Rust 编译检查
make build           # 构建前端产物
make tauri-build     # 构建完整 Tauri 桌面应用
make preview         # 预览构建后的前端
make clean           # 清理 Bun 缓存与 Rust target
```

### 默认验收入口

任务完成前默认运行：

```bash
make check-lint
```

小型文档改动可只运行与改动相关的检查，但汇报时必须明确说明没有运行完整门禁。涉及 Rust、IPC、前端类型或构建配置的改动，必须运行 `make check-lint`。

`make check-lint` 底层调用 `bash scripts/check_lint.sh`。Windows 环境需要可用的 Bash，例如 Git Bash、WSL Bash 或系统可发现的 `bash.exe`。

当前 `Makefile` 使用 `powershell.exe` 作为 shell，这是 Windows 优先项目的明确约束。非 Windows CI 或开发环境需要先提供 PowerShell，或新增独立的 POSIX Makefile / CI 脚本后再运行 make 目标。

## 架构概览

```text
DDNet-Manager/
├── src/                    # React 前端
│   ├── main.tsx            # 入口：挂载 React 到 #root
│   ├── App.tsx             # 当前主应用壳与首版 UI
│   ├── index.css           # Tailwind v4 + 工业霓虹设计系统类
│   └── vite-env.d.ts       # Vite 类型声明
├── src-tauri/              # Tauri / Rust 后端
│   ├── src/main.rs         # 当前后端入口：命令定义 + 事件发射
│   ├── icons/icon.ico      # Windows Tauri 图标
│   ├── Cargo.toml          # Rust 依赖
│   ├── Cargo.lock          # Rust 锁文件
│   ├── tauri.conf.json     # Tauri v2 配置
│   └── build.rs            # Tauri 构建脚本
├── scripts/check_lint.sh   # 统一工程门禁脚本
├── docs/superpowers/       # PRD 与实现计划
├── index.html              # HTML 入口
├── vite.config.ts          # Vite 配置
├── tsconfig*.json          # TypeScript 配置
├── Makefile                # 统一命令入口
├── bun.lock                # Bun 锁文件
└── package.json            # JS 依赖与脚本
```

## 核心架构模式

### Tauri IPC 通信

前端与后端的通信通过 Tauri IPC 桥接：

```typescript
const result = await invoke<ReturnType>("command_name");
const result = await invoke<ReturnType>("command_name", { arg1: value });
```

事件监听使用标准清理模式：

```typescript
useEffect(() => {
  let cleanup: UnlistenFn | undefined;
  void listen<PayloadType>("event_name", (event) => {
    // event.payload 是 PayloadType 类型
  }).then((fn) => {
    cleanup = fn;
  });
  return () => {
    cleanup?.();
  };
}, []);
```

当前已有命令仍是首版 mock / stub：

| 命令 | 用途 | 当前状态 |
| --- | --- | --- |
| `check_update` | 返回 `VersionInfo` | Mock，待接入自维护 manifest |
| `start_download` | 通过 `download-progress` 事件推送模拟进度 | 模拟，待接入真实下载事务 |
| `launch_game` | 启动 DDNet 客户端 | 存根，待接入本地客户端注册表 |

添加新 IPC 时必须同步：

- Rust command 函数和 `generate_handler!` 注册面。
- 前端 `invoke` 封装。
- 前后端共享类型。
- 对应测试或门禁检查。

### 后端模块拆分方向

当前 `src-tauri/src/main.rs` 可继续运行，但进入 MVP 实现后应按计划拆分：

1. `models.rs`：IPC 领域类型。
2. `commands.rs`：Tauri command 聚合层。
3. `client_scan.rs`：本地客户端识别。
4. `manifest.rs` / `download.rs`：更新源、下载、校验。
5. `cfg.rs` / `file_tx.rs`：Binds 解析、差异、备份、回滚。
6. `workshop.rs`：Workshop 静态 JSON adapter。
7. `process.rs`：启动与运行中检测。

不要使用 `mod.rs`。模块组织使用 `name.rs` + `name/` 子目录模式。

### 前端模块拆分方向

当前 `App.tsx` 是首版可运行 UI。进入 MVP 实现后按以下方向拆分：

1. 类型 → `src/types.ts`
2. IPC 封装 → `src/lib/tauri.ts`
3. 通用布局 → `src/components/layout/`
4. 启动页 → `src/components/launch/`
5. 客户端管理 → `src/components/clients/`
6. 更新管理 → `src/components/update/`
7. 资源位置 → `src/components/resources/`
8. Binds 管理 → `src/components/binds/`

`src/index.css` 只放 Tailwind 指令、全局变量和 `dm-*` 工具类。组件级样式使用 Tailwind 原子类。

## 产品约束

- QmClient-first，但模型必须能扩展到 DDNet 原版、TaterClient 等兼容客户端。
- 自维护 manifest 是更新权威源，GitHub 只承载下载资产。
- 默认网络策略是代理 / 镜像 URL，不默认修改 hosts。
- Everything / `es.exe` 只能作为可选扫描加速 provider，不能成为硬依赖。
- 客户端运行中禁止写 cfg，只允许只读分析和生成待应用计划。
- Binds 默认写入 Manager 专用 cfg，并使用可识别标记区块。
- 直接编辑 settings / autoexec 必须由用户显式选择，且写入前必须备份和展示 diff。

## 设计约束

- 窗口尺寸：1280×720。
- 应用风格：工业霓虹 / 暗黑 / Neon-Clean。
- 核心色：cyan `#41f2ff`，深黑背景 `#111213`。
- 字体族：Rajdhani → Bahnschrift → DIN Condensed → Segoe UI。
- 无原生窗口装饰，使用自定义标题栏。
- 标题栏拖拽区域必须使用 `data-tauri-drag-region`。
- 全局禁用文本选择：`user-select: none`。
- 全局隐藏滚动条。
- Tailwind CSS v4 使用 `@import "tailwindcss"`，不要改回 v3 的 `@tailwind base/components/utilities`。

## Rust 编码规约

- 新增 Rust 代码默认先写测试，除非是纯配置或生成文件。
- 公共 API、导出类型、Tauri command、跨模块复用入口必须写中文 `///` 文档注释。
- 非测试代码避免 `.unwrap()` / `.expect()`，优先使用 `?`、`map_err`、自定义错误类型。
- `unsafe` 必须有中文 `// SAFETY:` 注释，说明具体安全前提。
- 避免过深 `super::super::`，通过清晰 `use` 导入或模块边界解决。
- 单文件超过 600 行需要警惕，超过 1000 行属于门禁失败。
- 单函数超过 80 行需要拆分或解释，函数参数超过 4 个优先封装为结构体。
- 禁止 `todo!()`、`unimplemented!()`、空假实现和无期限占位。
- 涉及文件系统写入时必须考虑备份、原子性、失败恢复和路径边界。
- 涉及下载时必须考虑校验、临时文件、失败恢复和已有安装不被破坏。

## TypeScript / React 规约

- 新增代码禁止 `any`，优先使用明确 `interface` 或精确联合类型。
- 仅类型导入使用 `import type`。
- 前端调用 Tauri command 必须通过集中封装，避免组件散落裸 `invoke`。
- 不要默认引入新的状态库；现阶段 `useState` 足够，后续再基于真实复杂度决策。
- Framer Motion 用于关键交互动效，不要把所有 hover 都做成无意义动画。
- 保持 UI 的工业仪式感和机械感，不要退化成默认卡片式后台。

## 注释约束

- 不删除原有注释；只有注释与本次代码改动直接冲突时，才做最小同步修正。
- 新增或改写的注释默认使用中文。
- 修改带注释的代码时，必须检查注释是否仍然为真。
- 临时 workaround 注释必须写清触发条件、退出条件或后续清理位置。
- 坏注释禁止新增：逐行翻译代码、空话、过期解释、无期限“先这样”。

## 测试与验证

实现功能或修 bug 默认使用 TDD：

1. 先写失败测试。
2. 运行并确认因预期原因失败。
3. 写最少实现。
4. 运行测试确认通过。
5. 重构并保持测试通过。

任务完成前至少运行与改动相关的检查。涉及核心逻辑时运行：

```bash
make check-lint
```

`check_lint.sh` 当前覆盖：

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- 可选 `cargo audit`
- `bun install --frozen-lockfile`
- `bun run check`
- Rust 文件 / 函数 / 参数规模扫描
- 非测试 Rust `unwrap` / `expect` 告警
- `mod.rs` 禁用
- `super::super::` 告警
- 公共 API 文档注释告警
- `unsafe` SAFETY 注释告警
- TODO / FIXME / TBD / 假实现扫描

`FAIL` 必须修复后才能标记完成。`WARN` 若为本次改动引入，应处理；若为存量或可接受缺口，必须在汇报中说明。

## Git 与提交

- 可能存在用户或脚本产生的未提交改动。不要随意回退你没有计划动的文件。
- 不要使用 `git reset --hard` 或 `git checkout --` 回退文件，除非用户明确要求。
- 多主题改动尽量拆分提交。
- 提交文案默认遵循中文 Conventional Commits：

```text
<type>(<scope>): <中文动宾短语>
```

常用 type：`feat`、`fix`、`docs`、`refactor`、`perf`、`test`、`chore`、`ci`、`build`、`revert`。

示例：

```bash
git commit -m "build(lint): 接入 Rust 工程门禁脚本"
git commit -m "docs(agent): 同步项目代理规则"
```

## 子代理规则

用户明确要求“子代理”或任务影响核心逻辑时，才派发子代理。小任务不用审查，但以下情况完成后必须派只读子代理审查：

- 修改 Tauri IPC 契约、`generate_handler!` 注册面或前端 IPC 封装。
- 修改 Rust 下载、更新、文件写入、cfg 解析、进程检测等核心逻辑。
- 修改 `Makefile`、`scripts/check_lint.sh`、门禁策略或代理规则。
- 修改 Binds 写入策略、安全保护或回滚链路。
- 修改会影响用户数据目录、安装目录、资源目录识别的逻辑。

子代理要求：

- 子代理只做被分配的任务，不得再派发子代理。
- 审查子代理默认只读，不修改文件。
- 不要让子代理反问边界；任务描述必须给足上下文并要求直接产出报告。
- 子代理未返回最终报告前，不能声称审查完成。
- 子代理用完后及时关闭，避免挤占子代理数量池。
- 子代理审查应使用相关 Skills，例如 `/chinese-code-review`、`/code-review-excellence`。

## 工作方式

- 动手前先说清楚理解、假设和第一步。
- 有多种合理方案时，给出权衡和推荐，不要悄悄替用户做高风险选择。
- 外科手术式改动：每一行改动都应能追溯到任务目标。
- 不要顺手重构无关代码。
- 发现无关问题可以说明，但不要擅自修。
- 不要为了“未来可能需要”添加抽象、配置或依赖。
- 输出保持直接、具体、中文优先，不写空泛总结。
