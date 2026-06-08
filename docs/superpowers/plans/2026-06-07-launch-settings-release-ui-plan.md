# Launch Settings Release UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn launch settings, window close behavior, automatic update checking, and release automation into working behavior instead of UI placeholders.

**Architecture:** Extend the existing `AppSettings` JSON-backed model with two boolean fields, keep SQLite schema unchanged, and wire the settings through the existing Tauri IPC. Reuse the existing update-check command for automatic checks and adjust the launch page/titlebar to a game-launcher-style visual language.

**Tech Stack:** React 19, TypeScript 6, Tauri v2, Rust 2021, SQLite via rusqlite, GitHub Actions.

---

### Task 1: Persist Launch Settings

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/test/registry.rs`
- Modify: `src/types.ts`
- Modify: `src/App.tsx`

- [ ] Add failing Rust expectations for `close_panel_after_launch` and `auto_check_updates`.
- [ ] Run `make rust-test` and confirm the new expectations fail because fields are missing.
- [ ] Add the two fields to Rust and TypeScript `AppSettings` defaults with serde defaults.
- [ ] Run `make rust-test` and `make check`.

### Task 2: Make Settings Toggles Real

**Files:**
- Modify: `src/components/settings/SettingsDialog.tsx`

- [ ] Convert `TogglePill` into an accessible button-like switch surface.
- [ ] Wire the two launch settings to `onSettingsChange`.
- [ ] Keep the existing save button as the persistence action.

### Task 3: Wire Runtime Behavior

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/components/update/UpdatePanel.tsx`

- [ ] Add update-check state in `App`.
- [ ] On startup, after settings and default client are loaded, run `checkClientUpdate` when `auto_check_updates` is true.
- [ ] After successful game launch, minimize the main window when `close_panel_after_launch` is true.
- [ ] Surface automatic update status in the launch page compact update area.

### Task 4: Update Window and Launch UI

**Files:**
- Modify: `src/components/layout/TitleBar.tsx`
- Modify: `src/components/launch/LaunchPanel.tsx`

- [ ] Change the titlebar close button from `close()` to `minimize()`.
- [ ] Restyle window controls as floating game-overlay icon buttons with tooltips.
- [ ] Restyle launch primary action as a large yellow pill with a left icon.
- [ ] Add a clear secondary “已安装？定位游戏” action.

### Task 5: Add Release Workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`

- [ ] Add CI on push/PR to run `make check-lint`.
- [ ] Add release workflow on `v*` tags to run checks and `make tauri-build`.
- [ ] Upload Tauri bundle artifacts from `src-tauri/target/release/bundle`.

### Task 6: Verify and Review

- [ ] Run `make check-lint`.
- [ ] If UI changed, run a local dev build or server smoke check.
- [ ] Dispatch a read-only code review subagent because IPC/settings and CI/release behavior changed.
