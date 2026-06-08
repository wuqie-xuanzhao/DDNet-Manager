# DDNet Manager

[中文](#中文) | [English](#english)

## 中文

DDNet Manager 是一个面向 DDNet / QmClient 玩家的第三方桌面启动器与客户端管理器。项目目标不是替代 QmClient 的开发、构建或发布流程，而是为最终用户提供客户端发现、注册、启动、更新下载和安装管理能力。

当前项目处于早期 MVP 阶段。前端 UI、Tauri 壳、基础 IPC、客户端目录验证、manifest 读取、客户端注册表、真实下载任务和安装事务已经建立基础；自动扫描完善、设置持久化体验和下载/安装链路打磨仍在规划与实现中。

### 核心方向

- 客户端管理：识别本机 QmClient、DDNet Vanilla、QmClient Nightly 和其他第三方客户端。
- 启动管理：保存默认客户端、验证可启动状态并从 Manager 发起启动。
- 更新下载：通过自维护 manifest、官网或显式配置的镜像下载更新。
- 安装管理：校验下载资产、执行安装事务、记录安装历史并保留失败恢复信息。
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
- C：启动器体验打磨，包括默认客户端启动、自动检查更新、下载进度和安装历史展示。

相关文档：

- [PRD](docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md)
- [当前后端能力调研](docs/superpowers/explore/2026-06-07-当前后端能力调研.md)
- [下一步 MVP 完整规格](docs/superpowers/specs/2026-06-07-ddnet-manager-next-mvp-spec.md)
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

## English

DDNet Manager is a third-party desktop launcher and client manager for DDNet / QmClient players. It is not intended to replace the QmClient development, build, or release workflow. It focuses on end-user client discovery, registration, launching, update downloads, and installation management.

The project is currently in an early MVP stage. The Tauri shell, frontend UI, basic IPC, client directory validation, manifest loading, client registry, real download jobs, and installation transactions have initial implementations. Automatic scanning polish, settings persistence UX, and download/install flow hardening are still being designed and implemented.

### Product Direction

- Client management: detect QmClient, DDNet Vanilla, QmClient Nightly, and other compatible third-party clients.
- Launch management: save the default client, validate launch readiness, and launch from Manager.
- Update downloads: use a self-maintained manifest, official websites, or explicitly configured mirrors.
- Installation management: verify downloaded assets, run installation transactions, record install history, and keep failure recovery information.
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
- C: launcher experience polish, including default client launch, automatic update checks, download progress, and install history display.

Related documents:

- [PRD](docs/superpowers/specs/2026-06-06-ddnet-manager-prd.md)
- [Current backend capability research](docs/superpowers/explore/2026-06-07-当前后端能力调研.md)
- [Next MVP full spec](docs/superpowers/specs/2026-06-07-ddnet-manager-next-mvp-spec.md)
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
