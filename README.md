# DDNet Manager

[中文](#中文) | [English](#english)

## 中文

DDNet Manager 是一个面向 DDNet / QmClient 玩家的第三方桌面启动器与客户端管理器。项目目标不是替代 QmClient 的开发、构建或发布流程，而是为最终用户提供客户端发现、更新、资源路径管理、启动和 Binds 管理能力。

当前项目处于早期 MVP 阶段。前端 UI、Tauri 壳、基础 IPC、客户端目录验证、manifest 读取、cfg 解析和 Workshop JSON 拉取已经建立基础；真实下载、安装事务、回滚、自动扫描和设置持久化仍在规划与实现中。

### 核心方向

- 客户端管理：识别本机 QmClient、DDNet Vanilla、QmClient Nightly 和其他第三方客户端。
- 更新管理：通过自维护 manifest 从 GitHub 或显式配置的镜像下载更新。
- 资源管理：区分客户端安装目录、`data` 目录和用户数据目录。
- Binds 管理：分析本地 cfg、接入 Workshop，并在后续实现安全写入与回滚。
- Windows 优先：优先适配 Windows 用户路径、进程和本地客户端扫描。

### 技术栈

- Desktop：Tauri v2
- Backend：Rust 2021、Tokio、Reqwest、Serde
- Frontend：React 19、TypeScript、Vite、Tailwind CSS、Framer Motion
- Package manager：Bun
- Task runner：Make

### 开发命令

```powershell
make install
make dev
make tauri-dev
make check
make check-lint
make build
make tauri-build
```

### 当前后端优先级

当前后端强化规格采用 A+B 优先：

- A：客户端管理核心，包括扫描、识别、注册表、默认客户端和路径健康状态。
- B：真实更新下载链路，包括 manifest 选择、下载任务、sha256 校验、解压、安装事务和失败恢复。
- C：Binds 安全写入后置，包括 diff、备份、Manager 专用 cfg、运行中写入保护和回滚。

相关文档：

- [PRD](docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md)
- [后端强化探索](docs/superpowers/explore/2026-06-06-后端强化探索.md)
- [后端强化规格](docs/superpowers/specs/2026-06-06-backend-ab-strengthening.md)
- [MVP 实现计划](docs/superpowers/plans/2026-06-06-ddnet-manager-mvp-plan.md)

### 验证

主要验证入口是：

```powershell
make check-lint
```

该命令会统一运行 Rust 格式检查、Clippy、Rust 测试、TypeScript 类型检查、结构扫描和占位符扫描。`cargo audit` 可能报告 Tauri 上游依赖的 allowed warnings；只要汇总中 `FAIL: 0`，即可视为当前门禁通过。

### 关联项目

- QmClient：`E:\Coding\DDNet\QmClient`
- DDNet Bind Workshop：`https://ddrace.cn/bind`

## English

DDNet Manager is a third-party desktop launcher and client manager for DDNet / QmClient players. It is not intended to replace the QmClient development, build, or release workflow. It focuses on end-user client discovery, updates, resource path management, launching, and Binds management.

The project is currently in an early MVP stage. The Tauri shell, frontend UI, basic IPC, client directory validation, manifest loading, cfg parsing, and Workshop JSON loading have initial implementations. Real downloads, installation transactions, rollback, automatic scanning, and settings persistence are still being designed and implemented.

### Product Direction

- Client management: detect QmClient, DDNet Vanilla, QmClient Nightly, and other compatible third-party clients.
- Update management: use a self-maintained manifest to download updates from GitHub or explicitly configured mirrors.
- Resource management: distinguish the install directory, bundled `data` directory, and user data directory.
- Binds management: analyze local cfg files, connect to the Workshop, and later support safe writes and rollback.
- Windows first: prioritize Windows paths, processes, and local client scanning.

### Tech Stack

- Desktop: Tauri v2
- Backend: Rust 2021, Tokio, Reqwest, Serde
- Frontend: React 19, TypeScript, Vite, Tailwind CSS, Framer Motion
- Package manager: Bun
- Task runner: Make

### Development Commands

```powershell
make install
make dev
make tauri-dev
make check
make check-lint
make build
make tauri-build
```

### Current Backend Priorities

The backend strengthening plan prioritizes A+B:

- A: client management core, including scanning, classification, registry persistence, default client selection, and path health state.
- B: real update and download flow, including manifest resolution, download jobs, sha256 verification, extraction, installation transactions, and failure recovery.
- C: Binds safe writing is planned later, including diff preview, backup, Manager-owned cfg blocks, running-client write protection, and rollback.

Related documents:

- [PRD](docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md)
- [Backend exploration](docs/superpowers/explore/2026-06-06-后端强化探索.md)
- [Backend strengthening spec](docs/superpowers/specs/2026-06-06-backend-ab-strengthening.md)
- [MVP implementation plan](docs/superpowers/plans/2026-06-06-ddnet-manager-mvp-plan.md)

### Verification

The main verification entry point is:

```powershell
make check-lint
```

It runs Rust formatting checks, Clippy, Rust tests, TypeScript checks, structural scans, and placeholder scans. `cargo audit` may report allowed warnings from upstream Tauri dependencies; the current gate is acceptable when the summary reports `FAIL: 0`.

### Related Projects

- QmClient: `E:\Coding\DDNet\QmClient`
- DDNet Bind Workshop: `https://ddrace.cn/bind`
