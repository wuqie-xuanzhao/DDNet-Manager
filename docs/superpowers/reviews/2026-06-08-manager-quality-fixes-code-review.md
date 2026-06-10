---
commit: a3912c8df8b9fc7b81cce01e3906a108e7bf659b
review_type: deep
scope: origin/main...HEAD
status: completed
created_at: 2026-06-08
---

# manager-quality-fixes 代码审查报告

## 摘要

本次只读审查覆盖 `origin/main...HEAD`，当前提交为 `a3912c8df8b9fc7b81cce01e3906a108e7bf659b`。变更范围较大，涉及 Tauri 后端更新/下载/安装链路、前端启动与更新流程、CI/release、门禁脚本和本地 Tauri smoke 验收。

整体方向符合 MVP 质量强化目标，但当前仍有阻塞级问题：smoke 更新页把未落库的临时客户端 ID 交给后端下载入口，真实 smoke 会在下载阶段找不到安装记录；CI/release 也会在 frozen lockfile 检查前运行可修改锁文件的 `make install`，削弱锁文件门禁。

## 审查范围

- 后端核心：`src-tauri/src/commands.rs`、`download.rs`、`registry.rs`、`manifest.rs`、`models.rs`、`local_smoke.rs` 及相关 Rust 测试。
- 前端流程：`src/App.tsx`、`src/components/update/UpdatePanel.tsx`、`src/components/launch/LaunchPanel.tsx`、`src/hooks/useClientLauncher.ts`、`src/hooks/useAutoUpdate.ts`、`src/lib/tauri.ts`、`src/lib/updateLogic.ts`、`src/types.ts` 及相关测试。
- 工程门禁：`.github/workflows/ci.yml`、`.github/workflows/release.yml`、`Makefile`、`scripts/check_lint.sh`、`scripts/tauri_update_smoke.ps1`、`package.json`、`vite.config.ts`、`eslint.config.js`、README/AGENTS/CLAUDE 命令规则。

已按项目规则派发 3 个只读子代理分别审查后端、前端、工程门禁。三个子代理均已返回报告，并已关闭。

## Findings

| 级别 | 位置 | 问题 |
| --- | --- | --- |
| [必须修复] | `src/components/update/UpdatePanel.tsx:248`, `src-tauri/src/commands.rs:265` | smoke 更新页使用未落库的临时客户端启动下载，真实 smoke 会找不到安装记录 |
| [必须修复] | `.github/workflows/ci.yml:63`, `.github/workflows/release.yml:64` | CI/release 在 frozen lockfile 检查前运行 `make install`，可能掩盖 `bun.lock` 漏提交 |
| [建议修改] | `src/components/update/UpdatePanel.tsx:488`, `src-tauri/src/commands.rs:221` | 更新检查按默认客户端推导版本，不能准确检查当前安装记录 |
| [建议修改] | `src/lib/tauri.ts:85` | 启动 readiness 可能把已损坏的旧默认记录显示为“可启动” |
| [建议修改] | `src/components/launch/LaunchPanel.tsx:150` | `running` 状态仍会再次调用启动命令 |
| [建议修改] | `src-tauri/src/commands.rs:650` | 安装成功后的任务状态/历史持久化失败处理不一致 |
| [建议修改] | `scripts/tauri_update_smoke.ps1:329` | smoke 脚本没有总超时，状态机卡住时会永久挂起 |

## 详细问题

### [必须修复] smoke 临时客户端未落库导致下载阶段找不到安装记录

证据：

- `src/components/update/UpdatePanel.tsx:248-250` 在 smoke 模式下通过 `validateClientDir(smokeClientInstallDir)` 得到 `nextClient`。
- `src/components/update/UpdatePanel.tsx:577-583` 下载时把 `client.id` 作为 `client_installation_id` 传给 `startUpdateDownload`。
- `src-tauri/src/commands.rs:265-269` 后端下载入口只从 SQLite 注册表 `list_client_installations()` 中查找该安装记录。
- `src/hooks/useClientLauncher.ts:231-233` smoke bootstrap 使用 `persistDefault: false`，不会把该客户端保存为默认客户端。

失败场景：

本地 smoke 启动后，启动页验证临时客户端目录并跳到更新页；更新页再次用 `validateClientDir()` 构造一个临时 `ClientInstallation`，但没有将它保存到注册表。检查更新可以成功，因为只需要 `client_id`；一旦进入下载，后端按 `client_installation_id` 查注册表，找不到该临时安装记录，返回 `client installation not found`，真实 smoke 自动更新链路无法完成。

建议修复：

让 smoke 客户端以非默认临时记录落库，或给后端提供明确的 smoke-only 安装目标通道。修复后补一条端到端或集成测试，覆盖 smoke 更新流程中 `start_update_download` 能找到该临时安装记录。

### [必须修复] CI/release 会在锁文件一致性检查前运行可修改锁文件的安装

证据：

- `.github/workflows/ci.yml:63-67` 先运行 `make install`，再运行 `make check-lint`。
- `.github/workflows/release.yml:64-68` 同样先运行 `make install`，再运行 `make check-lint`。
- `Makefile:8-9` 的 `make install` 是普通 `bun install`，不是 frozen install。
- `scripts/check_lint.sh` 的 lockfile 检查是 `bun install --frozen-lockfile`，但此时工作区锁文件可能已经被前一步修改。

失败场景：

PR 修改 `package.json` 但漏提交 `bun.lock`。GitHub Actions 先执行普通 `bun install`，可能在 CI 工作区更新锁文件；随后 `make check-lint` 内的 frozen install 基于已被更新的本地 `bun.lock` 通过，导致锁文件一致性门禁失效。

建议修复：

CI/release 不要在门禁前运行 `make install`。可以直接运行 `make check-lint`，或新增 `make install-frozen` 并在 workflow 中使用 frozen install。release 构建若需要安装依赖，也应使用 frozen 入口。

### [建议修改] 更新检查按默认客户端版本判断，不适合当前安装记录

证据：

- `src/components/update/UpdatePanel.tsx:488-494` 前端检查更新只传 `client_id`、`channel`、manifest 和网络路由。
- `src-tauri/src/commands.rs:221-229` 后端按“同 `client_id` 且 `is_default`”的客户端推导 `current_version`。

失败场景：

用户有两个 QmClient 安装，默认客户端是 `1.0.0`，当前更新页目标安装记录是 `0.9.0`；后端仍会拿默认客户端版本判断更新，导致当前目标的更新提示不准确。smoke 模式也会受影响：临时客户端没有落库版本，检查更新仍可能被持久化默认客户端版本污染。

建议修复：

扩展 IPC 请求，传 `client_installation_id` 或显式 `current_version`；后端按指定安装记录计算当前版本。对应补测试断言非默认安装记录和 smoke 临时记录不会读取默认客户端版本。

### [建议修改] 启动 readiness 未重新验证默认客户端目录

证据：

- `src/lib/tauri.ts:61-94` `getLaunchReadiness()` 读取默认客户端后直接使用存量 `health` 和 `compatibility`。
- `src/lib/tauri.ts:85-91` `isClientRunning()` 失败时只把错误加入 `blocking_reasons`，没有让 `canLaunch` 变为 false。

失败场景：

默认客户端记录仍是旧的 `health: "ok"`，但用户删除了 `DDNet.exe`。运行检测因路径不存在失败后，UI 仍可能显示“可启动”，因为 `canLaunch` 只看旧记录的 `health` 与 `compatibility`。

建议修复：

读取默认客户端后重新调用后端 `validate_client_dir`，或至少在运行检测失败代表路径/可执行文件不可用时把 `can_launch` 置为 false，并显示错误状态。

### [建议修改] `running` 状态仍会再次启动客户端

证据：

- `src/components/launch/LaunchPanel.tsx:143` `primaryDisabled` 没有禁用 `running`。
- `src/components/launch/LaunchPanel.tsx:150-152` `ready` 和 `running` 都会调用 `onPrimaryAction()`。
- `src/hooks/useClientLauncher.ts:325-327` `onPrimaryAction()` 会调用 `launchDefaultClient()`。

失败场景：

客户端已经运行时，启动页状态为 `running`，用户点击主按钮会再次调用启动命令，可能重复拉起客户端或触发后端启动错误。

建议修复：

`running` 状态下主按钮应禁用并展示“已运行”，或改为执行聚焦已有窗口/最小化 Manager，不再调用 `launchDefaultClient()`。

### [建议修改] 安装成功后的元数据持久化失败处理不一致

证据：

- `src-tauri/src/commands.rs:650` `complete_download_job_snapshot()` 会先修改内存任务状态，再写入注册表；写入失败直接 `?` 返回。
- `src-tauri/src/commands.rs:651-659` `record_install_history()` 的结果被 `let _ =` 忽略。

失败场景：

文件替换和客户端记录更新已经完成，但 SQLite 写 `download_jobs` 或 `install_history` 失败。此时用户可能看到命令错误、内存状态为 completed、数据库仍是 installing，或安装历史完全缺失。重启后恢复视图和安装历史会不一致。

建议修复：

把客户端记录、下载任务状态、安装历史放进同一元数据事务；或采用类似 `enter_installing_snapshot()` 的回滚策略。至少不要静默吞掉安装历史写入失败，应暴露“安装成功但历史记录失败”的明确状态。

### [建议修改] Tauri smoke 脚本没有总超时

证据：

- `scripts/tauri_update_smoke.ps1:329` 直接执行 `bun run tauri dev --config ...`，脚本等待该进程退出。
- 结果断言在进程退出后才执行。

失败场景：

前端 smoke 状态机卡住、窗口没有关闭、IPC 结果文件没有写出时，`tauri dev` 会持续运行，脚本永远到不了 `Assert-LocalSmokeSucceeded`，本地或 CI smoke 会挂住而不是失败退出。

建议修复：

用 job 或 `Start-Process` 包装 Tauri dev，增加总超时。超时后停止子进程，输出 stdout/stderr tail，并以失败退出。

## 回归风险

- 已验证：代码结构和 IPC 关键路径已通过只读 diff、CodeGraph、文件读取进行人工交叉检查。
- 未验证：没有运行 `make check-lint`，因此 Rust clippy/test、前端 Vitest/ESLint、TypeScript build 当前状态未在本轮确认。
- 未验证：没有运行 `make tauri-smoke-update`，因此真实 Tauri 更新 smoke 成功路径未在本轮确认。
- 未验证：没有启动桌面应用，因此运行态按钮、窗口关闭、下载进度事件和安装事件没有做人工 UI 验证。

## 验证记录

已执行只读检查：

- `git status --short`
- `git status --branch --short`
- `git log --oneline --decorate -n 8`
- `git diff --stat origin/main...HEAD`
- `git diff --name-status origin/main...HEAD`
- `git diff --numstat origin/main...HEAD`
- CodeGraph status/explore 查询
- 关键文件只读行号核对
- 子代理只读审查：后端、前端、工程门禁

未执行：

- `make check-lint`
- `make tauri-smoke-update`
- `cargo test`
- `bun run test`
- `bun run lint`

原因：用户要求“只读，不修改”。上述命令会写入依赖、缓存、`src-tauri/target`、`tmp` 或其他构建产物，不满足只读约束。

## 肯定

- 后端新增下载任务持久化、恢复摘要、安装历史和本地 smoke 报告入口，方向上补齐了 MVP 更新链路的关键可观测性。
- 前端把启动、设置、自动更新逻辑拆到 hooks 和 lib 中，比单一 `App.tsx` 更易测试。
- 新增 Vitest/ESLint、GitHub Actions 和 smoke 脚本，能明显提高后续质量门禁能力；需要先修复上述门禁顺序和 smoke 闭环问题。

## 建议处理顺序

1. 先修复 smoke 临时客户端落库/后端下载契约，否则 `make tauri-smoke-update` 的核心目标无法稳定成立。
2. 调整 CI/release 的安装顺序，确保 lockfile 检查不可被普通 install 掩盖。
3. 扩展更新检查 IPC，使版本判断绑定到具体安装记录。
4. 修复启动 readiness 和 `running` 主按钮行为。
5. 为安装成功后的元数据持久化失败补事务或明确状态。
6. 给 smoke 脚本增加总超时和失败诊断输出。
