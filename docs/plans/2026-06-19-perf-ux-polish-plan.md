# DDNet Manager 性能与 UX 打磨 Sprint Plan

**日期**: 2026-06-19
**作者**: Proma Agent + 鸢天
**预估工期**: 4-6 个工作日
**任务来源**: 基于 code review + explorer 三路诊断（启动速度 / 扫描进度 / 下载安装 / 交互视觉）

## 背景与目标

启动器重设计 PRD（`docs/plans/2026-06-18-mihoyo-launcher-redesign.md`）+ 验收清单（`docs/plans/2026-06-19-launcher-acceptance-checklist.md`）的代码层面已 100% 完成。本 sprint 处理"功能有了但体验还不够好"的剩余痛点，分四个方向：

- **启动速度**：catalog/release/自更新拉取链路串行 + 不必要延迟
- **扫描进度**：priority 阶段黑屏 + events 无界增长 + 扫描后状态抖动
- **下载/安装**：进度节流 + 校验进度 + 错误友好化 + 快捷方式失败被吞
- **交互视觉**：toast 缺失 + Esc 不统一 + 快捷键 + 4K 缩放 + 主题切换

## 15 项痛点清单（按阶段分组）

### 阶段 A：基础设施（先行，给后续铺路）

| # | 任务 | 文件 | 依赖 |
|---|---|---|---|
| A1 (#19) | 引入 sonner + useToast 封装 | `src/components/ui/sonner.tsx` 新建、`src/lib/toast.ts` 新建 | 无 |
| A2 (#20) | 抽 usePopoverState 统一 Esc + backdrop 关闭 | `src/hooks/usePopoverState.ts` 新建 | 无 |

### 阶段 B：扫描体验（用户感知最强）

| # | 任务 | 文件 | 依赖 |
|---|---|---|---|
| B1 (#13) | run_scan 开头 emit phase_started 事件 | `src-tauri/src/commands/scan.rs` | 无 |
| B2 (#14) | triggerScan 暴露 scan-progress 给 InstallDialog | `src/hooks/useClientInstaller.ts`、`InstallDialog.tsx` | 无 |
| B3 (#15) | events 数组 cap 到最近 50 条 | `src/hooks/useClientScanner.ts` | 无 |
| B4 (#23) | 扫描 priority 命中时 Rust 端自动 upsert | `src-tauri/src/commands/scan.rs`、`useClientInstaller.ts` | 无 |

### 阶段 C：下载/安装体验

| # | 任务 | 文件 | 依赖 |
|---|---|---|---|
| C1 (#16) | verify-downloaded-file emit verify-progress | `src-tauri/src/download/verify.rs`、`DownloadButton.tsx` | 无 |
| C2 (#21) | 下载进度事件节流（Rust 100ms/1% + 前端 rAF） | `src-tauri/src/commands/download.rs`、`useClientInstaller.ts` | 无 |
| C3 (#17) | 下载/校验错误码 → 中文友好消息映射 | `src-tauri/src/download/error.rs` 新建、`useClientInstaller.ts` | A1 |
| C4 (#22) | createShortcuts 结果回传 + 失败 toast | `useClientInstaller.ts`、`InstallDialog.tsx` | A1 |

### 阶段 D：启动速度

| # | 任务 | 文件 | 依赖 |
|---|---|---|---|
| D1 (#18) | useAppUpdater AUTO_CHECK_DELAY_MS 1500 → 300 | `src/hooks/useAppUpdater.ts` | 无 |
| D2 (#24) | refreshFromRegistry 与 fetchRelease 并行 | `src/hooks/useClientInstaller.ts`、`App.tsx` | 无 |

### 阶段 E：交互视觉细节

| # | 任务 | 文件 | 依赖 |
|---|---|---|---|
| E1 (#25) | 抽 useHotkey 加全局快捷键 | `src/hooks/useHotkey.ts` 新建、`App.tsx` | 无 |
| E2 (#26) | 4K 缩放下固定 px 改 rem 或 style | `DownloadButton.tsx`、`SettingsDialog.tsx` 等 | 无 |
| E3 (#27) | 主题切换 CSS transition 平滑过渡 | `src/index.css` | 无 |

## 实施顺序与依赖

```
阶段 A（基础设施，先行）
  ├─ A1 sonner toast ──┐
  └─ A2 usePopoverState│
                       │
阶段 B（扫描）         │  阶段 D（启动速度）
  ├─ B1 phase_started  │    ├─ D1 自更新延迟（独立）
  ├─ B2 InstallDialog  │    └─ D2 catalog 并行（独立）
  ├─ B3 events cap     │
  └─ B4 priority upsert│  阶段 E（交互视觉）
                       │    ├─ E1 useHotkey（独立）
阶段 C（下载）         │    ├─ E2 4K rem（独立）
  ├─ C1 verify-progress│    └─ E3 主题 transition（独立）
  ├─ C2 节流           │
  ├─ C3 错误友好化 ────┤ 依赖 A1
  └─ C4 快捷方式 toast ┤ 依赖 A1
```

**关键约束**：
- A1 必须在 C3/C4 之前做（toast 系统）
- 阶段 B / D / E 内部各项独立，可任意顺序
- 单文件多处改动需协调（如 `useClientInstaller.ts` 被 B2/C2/C4/D2 都改）—— 推荐**每完成 1-2 项就 commit**，避免最后大冲突

## 验收标准

每项任务完成后必须：
1. **代码层**：`make check-lint` 全绿（PASS / WARN 不增长 / FAIL 0）
2. **行为层**：手动跑 `make tauri-dev`，按 task description 走一遍验收路径
3. **commit**：遵循中文 Conventional Commits，type 用 `perf`/`feat`/`refactor`/`fix` 视改动性质

## 风险与回退

- **C2 下载节流** 改了 Rust + 前端双端，如果节流阈值不当（太低导致进度跳动 / 太高导致进度卡顿），需调参；建议保留原始 emit 频率作 feature flag 一周观察后再删
- **B4 priority 自动 upsert** 改了 scan 行为，如果 priority 误判（用户曾装过又删了，残留目录）会污染 registry；需在 upsert 前加 health check
- **E2 4K rem** 涉及多个文件、可能视觉偏差，做完需在 100%/150%/175% 缩放下各跑一次

## 不在本 sprint 范围

以下 7 项痛点诊断列出但**不做**（独立大项目或 NIT）：

- **断点续传**（#16）：HTTP Range/If-Range + .part 保留，独立 plan
- **i18n 框架**（#17）：i18next 接入 + 字符串抽取，独立 plan
- **NIT 5 项**：catalog cache 复用、priority roots 缓存、下载速度字段、加载状态视觉统一、Tooltip focus-within —— 后续顺手清

## 进度跟踪

- Task list：本会话 `TaskList` 的 #12-#27（15 项 + plan 文档）
- 每完成一项：`TaskUpdate` 标 completed + 提交 commit + 在本文档对应行加 ✅
