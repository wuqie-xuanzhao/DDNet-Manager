---
type: question
status: current
confidence: high
scope: src, scripts/check_lint.sh, react-doctor diagnostics
commit: a7e6281
created: 2026-06-08
---

# React Doctor 剩余问题与前端行数门禁探索

## Quick Answer

本轮已清理 React Doctor 中收益明确、风险较低的问题：未声明 `button type`、无可访问名称的输入控件、灰色文字压在彩色背景、以及 `dangerouslySetInnerHTML` 注入本地 SVG。复跑 `npx react-doctor@latest --verbose` 后，总问题从 54 降到 31；其中 `require-reduced-motion` 是明确暂缓项，排除后剩 30 个。

剩余 30 个不适合继续用“机械快修”处理，主要是状态结构、组件拆分、Framer Motion 加载策略、IPC 封装预留导出和事件/effect 边界问题。React/TSX 的行数也不应套 Rust 的硬阈值；`check_lint.sh` 已把单文件行数门禁明确限定为 Rust 文件，前端复杂度以后优先看 React Doctor、ESLint、状态/effect 数量和职责边界。

## Key Evidence

1. 最新 React Doctor 输出目录是 `C:\Users\11054\AppData\Local\Temp\react-doctor-c6e4b93d-c34c-4816-867c-077bac8df75b`，`diagnostics.json` 统计为 31 项；排除用户要求先放过的 `react-doctor/require-reduced-motion` 后为 30 项。

2. 本轮已消除的规则包括 `button-has-type`、`control-has-associated-label`、`no-gray-on-colored-background` 和 `no-danger`。对应修复点在 `src/components/clients/ClientManager.tsx`、`src/components/layout/TitleBar.tsx`、`src/components/update/UpdatePanel.tsx`、`src/components/settings/SettingsDialog.tsx` 和 `src/components/icons/GameIcon.tsx`。

3. `src/components/icons/GameIcon.tsx` 原先通过 `dangerouslySetInnerHTML` 渲染 `*.svg?raw` 资源；React Doctor 官方说明指出该 API 会绕过 JSX 自动转义。本轮已改为 lucide React 图标映射，保留 `GameIcon` / `GameIconName` 调用接口。

4. 最新剩余规则按数量排序：`async-defer-await` x6、`unused-export` x5、`use-lazy-motion` x4、`prefer-useReducer` x3、`no-cascading-set-state` x2、`no-giant-component` x2、`no-render-in-render` x2、`unused-file` x2，以及 `exhaustive-deps`、`jsx-no-jsx-as-prop`、`no-event-handler`、`rerender-state-only-in-handlers` 各 1。

5. `scripts/check_lint.sh` 的结构扫描本来只通过 `all_rs()` 扫描 Rust 文件，但变量名和标题使用泛化的“单文件行数”。本轮改成 `MAX_RUST_FILE_LINES` / `HARD_MAX_RUST_FILE_LINES`，并把 C1 标题改成 “Rust 单文件行数”，避免误导为 React/TSX 也按同一硬阈值评价。

## Remaining Triage

### 优先级 A：需要独立小重构

- `exhaustive-deps` at `src/components/update/UpdatePanel.tsx:222`
  - 影响：cleanup 可能读取变化后的 ref，事件或当前客户端上下文清理可能错位。
  - 建议：先把 hydrate effect 的当前客户端 ID 快照和 cleanup 逻辑梳理清楚，再改依赖，不要机械补 deps。

- `no-event-handler` at `src/components/update/UpdatePanel.tsx:629`
  - 影响：smoke 自动流程通过 state + effect 驱动，可能晚一拍并多一次渲染。
  - 建议：把自动 smoke 的阶段推进抽成显式状态机或命令函数，避免 effect 充当事件处理器。

- `rerender-state-only-in-handlers` at `src/components/update/UpdatePanel.tsx:139`
  - 影响：`hydratedKey` 只用于控制流程，不显示在 UI 上，用 state 会触发无意义重绘。
  - 建议：配合 smoke/hydrate 状态机一起改成 ref 或 reducer 字段。

### 优先级 B：结构债，需要分阶段

- `prefer-useReducer` x3 / `no-cascading-set-state` x2 / `no-giant-component` x2
  - 影响：`UpdatePanel`、`ClientManager`、`App` 的状态分散，改动时容易遗漏某个 state 或 effect。
  - 建议：不要一次性全仓 reducer 化。先从 `UpdatePanel` 开始，把下载任务、安装、恢复、smoke、路由设置拆成小 reducer 或 hook；`SettingsDialog` 374 行只是提醒，不应按 Rust 行数规则硬拆。

- `no-render-in-render` x2
  - 影响：`renderActiveView()` / `renderSection()` 这种内联渲染函数可能导致局部子树重挂载，用户正在输入时可能丢局部状态。
  - 建议：提取命名组件时保持 props 明确，避免顺手引入全局状态库。

### 优先级 C：性能和工程清理

- `use-lazy-motion` x4
  - 影响：Framer Motion 全量导入增加前端包体；Tauri 桌面影响小于 Web，但仍是合理优化。
  - 建议：统一引入 `LazyMotion` + `m`，并和 `require-reduced-motion` 一起处理，避免动画策略分裂。

- `async-defer-await` x6
  - 影响：部分跳过路径等待了不需要的 IPC，可能让交互慢一点。
  - 注意：这些命中多处包含“await 后检查 requestId / alive”的过期请求保护，不能机械把 await 移到 guard 后。

- `unused-export` x5 / `unused-file` x2
  - 影响：维护噪音，不直接影响用户。
  - 建议：先确认 `src/lib/tauri.ts` 的导出是否是未来 IPC 封装预留；shadcn/ui 的 `collapsible.tsx`、`separator.tsx` 若确认为未用再删除。

## 行数门禁结论

Rust 的单文件/单函数行数门禁仍然合理，因为 Rust 后端模块边界、公共 API、文件事务和 IPC 注册面更接近“代码逻辑密度”的风险。React/TSX 则混合了 JSX 模板、布局样式、动效和局部状态，单纯行数不能可靠代表风险。

前端复杂度更合适的判断信号是：

- React Doctor 的 `no-giant-component`、`prefer-useReducer`、`no-cascading-set-state`。
- ESLint React Hooks 规则。
- 状态数量、effect 数量、是否同时承担多个业务职责。
- 用户输入和异步事件是否可能丢状态、晚响应或重复渲染。

因此，`check_lint.sh` 不应新增 TSX 硬行数失败规则；最多可以把 React Doctor 或 ESLint 作为前端结构信号。

## Follow-up Plan

1. 单独处理动画策略：`require-reduced-motion` + `use-lazy-motion` 一起做，避免只减包体但不解决 WCAG。
2. 单独处理 `UpdatePanel` 状态机：先 hydrate/smoke/download/install，再考虑 reducer。
3. 审核 `src/lib/tauri.ts` 未用导出和 shadcn/ui 未用文件，确认不是预留后再删。
4. 把 `renderActiveView`、`renderSection` 提取为命名组件，重点验证输入和局部 UI 状态不丢。
