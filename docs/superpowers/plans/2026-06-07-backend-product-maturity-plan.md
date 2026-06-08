# Backend Product Maturity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 围绕当前已存在的 Tauri 下载/安装后端，补齐安装事件前端闭环、安装历史/回滚路径 UI、下载任务持久化与恢复，以及真实 Tauri 端到端验证，使更新链路从“代码上可用”推进到“用户可感知、可恢复、可验收”。

**Architecture:** 保持 `src-tauri` 作为唯一后端边界：下载任务继续由 `DownloadManager` 驱动实时状态，但每次状态变化都同步写入 SQLite 注册表，前端通过新的恢复查询 IPC 读取“当前任务 + 历史记录 + 恢复建议”。前端不扩散到新的导航页面，继续以 `src/components/update/UpdatePanel.tsx` 作为下载、安装、历史与恢复入口，因为 `src/App.tsx` 已经提供独立“更新”视图。真实端到端验证先补可重复的 smoke 场景和 `make` 入口，不额外引入新的前端测试框架。

**Tech Stack:** Tauri v2 + Rust 2021 + tokio + reqwest + serde + rusqlite；React 19 + TypeScript 6 + Vite 8 + Tailwind CSS v4；统一命令入口使用 `make`。

---

## Planning Inputs

- `docs/superpowers/explore/2026-06-07-当前后端能力调研.md`
- `src-tauri/src/commands.rs`
- `src-tauri/src/download.rs`
- `src-tauri/src/registry.rs`
- `src-tauri/src/models.rs`
- `src/components/update/UpdatePanel.tsx`
- `src/lib/tauri.ts`
- `src/types.ts`
- `Makefile`
- `package.json`

## Current Code Anchors

1. 后端已经发出 `install-progress`、`install-completed`、`install-failed` 事件，但更新页当前只监听 `download-progress`、`download-completed`、`download-failed`。
2. `UpdatePanel` 已经具备 `installing` / `completed` 状态文案，但没有消费安装阶段事件。
3. `src/lib/tauri.ts` 已经封装 `listInstallHistory()`，但前端没有展示安装历史或回滚路径。
4. 下载任务当前仍主要存在于 `DownloadManager` 的进程内 `HashMap`，`registry.rs` 尚未提供下载任务持久化台账。
5. 仓库当前没有 Vitest/Jest 等前端测试运行器，因此本计划不新增前端单测框架；验证依赖 Rust 测试、`make check` / `make check-lint` 与真实 Tauri smoke 验收。

## Scope Boundaries

- 只做以下四个方向：
  1. 安装事件监听与前端反馈闭环
  2. 安装历史 / 回滚路径 UI
  3. 下载任务持久化与恢复策略
  4. 真实 Tauri 端到端验证
- 不扩展到 `ClientManager`、`LaunchPanel`、`SettingsDialog` 的产品重做。
- 不改 `src/App.tsx` 导航结构，除非实现时发现更新页无法承载新增恢复区块；默认不改。
- 不顺带做结构化错误码体系重构；当前阶段只补最小必要的用户可读恢复文案。
- 不恢复 Workshop / Binds / 资源管理 / cfg 写入相关范围。

## Dependency Order

| 顺序 | 任务 | 依赖关系 | 原因 |
|---|---|---|---|
| 1 | 下载任务持久化与恢复策略 | 无 | 这是重启恢复、历史 UI 和 smoke 验收的基础 |
| 2 | 安装事件监听与前端反馈闭环 | 建议在 1 完成后开始 | 事件监听本身可独立做，但与恢复态展示共享 `UpdatePanel` 状态模型 |
| 3 | 安装历史 / 回滚路径 UI | 依赖 1；可与 2 交叉 | 历史与恢复入口都应落在更新页，避免二次改布局 |
| 4 | 真实 Tauri 端到端验证 | 依赖 1-3 | 只有前三项落地后，端到端链路才有完整验收意义 |

## File Structure

### Backend

- Modify: `src-tauri/src/models.rs`
  - 增加下载恢复摘要类型，作为前后端共享 IPC 契约。
- Modify: `src-tauri/src/download.rs`
  - 增加“缓存文件恢复状态”纯函数，保持恢复判断可测试。
- Modify: `src-tauri/src/registry.rs`
  - 增加下载任务台账 schema 和读写 API。
- Modify: `src-tauri/src/commands.rs`
  - 在下载任务状态迁移时写入注册表；新增恢复查询 / 清理命令；规范安装事件 payload。
- Modify: `src-tauri/src/main.rs`
  - 注册新增 IPC。
- Test: `src-tauri/src/test/models.rs`
- Test: `src-tauri/src/test/download.rs`
- Test: `src-tauri/src/test/registry.rs`
- Test: `src-tauri/src/test/commands.rs`

### Frontend

- Modify: `src/types.ts`
  - 镜像新增恢复类型。
- Modify: `src/lib/tauri.ts`
  - 集中封装新增恢复查询 / 清理 IPC。
- Modify: `src/components/update/UpdatePanel.tsx`
  - 监听安装事件，展示恢复区、安装历史和回滚路径。
- No change expected: `src/App.tsx`
  - 当前已有 `update` 视图，本计划默认不新增导航。

### Smoke Validation

- Modify: `Makefile`
  - 增加真实 Tauri 更新 smoke 命令入口。
- Create: `scripts/tauri_update_smoke.ps1`
  - 准备本地 fixture、启动本地 manifest 服务、拉起 Tauri dev，并输出人工验收步骤。
- Create: `src-tauri/src/test/fixtures/update-smoke/manifest.json`
  - 本地 smoke manifest。
- Create: `src-tauri/src/test/fixtures/update-smoke/package-root/`
  - 生成 smoke 更新包所需的最小客户端目录结构。

## Task 1: Persist and Recover Download Jobs

**Goal:** 应用重启后，前端能重新读取下载任务台账，并区分“可继续安装 / 需要重下 / 缓存损坏 / 已中断失败”四类恢复状态。

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/download.rs`
- Modify: `src-tauri/src/registry.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/types.ts`
- Modify: `src/lib/tauri.ts`
- Test: `src-tauri/src/test/models.rs`
- Test: `src-tauri/src/test/download.rs`
- Test: `src-tauri/src/test/registry.rs`
- Test: `src-tauri/src/test/commands.rs`

- [ ] **Step 1: 先补 Rust 恢复模型失败测试**

Add to `src-tauri/src/test/models.rs`:

```rust
#[test]
fn serializes_download_job_recovery() {
    let recovery = crate::models::DownloadJobRecovery {
        job: crate::models::DownloadJob {
            id: "download-1".to_string(),
            client_installation_id: "qmclient-main".to_string(),
            client_id: "qmclient".to_string(),
            channel: "stable".to_string(),
            version: "2.62.4".to_string(),
            asset_url: "https://example.invalid/qmclient.zip".to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            size: 42,
            status: crate::models::DownloadJobStatus::Verified,
            downloaded_bytes: 42,
            cache_path: "C:/tmp/qmclient.zip".to_string(),
            error: None,
        },
        cache_state: crate::models::DownloadCacheState::Verified,
        can_install: true,
        can_retry: false,
        user_message: "下载已校验，可继续安装。".to_string(),
    };

    let value = serde_json::to_value(recovery).expect("恢复模型应可序列化");
    assert_eq!(value["cache_state"], "verified");
    assert_eq!(value["can_install"], true);
    assert_eq!(value["can_retry"], false);
}
```

- [ ] **Step 2: 运行测试确认当前缺口**

Run:

```powershell
make rust-test
```

Expected: FAIL，因为 `DownloadJobRecovery` / `DownloadCacheState` 尚不存在。

- [ ] **Step 3: 在共享模型中补恢复摘要类型，并同步前端镜像类型**

Add to `src-tauri/src/models.rs`:

```rust
/// 表示下载缓存文件在应用重启后的恢复状态。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadCacheState {
    Missing,
    Present,
    Verified,
    Corrupted,
}

/// 表示前端恢复下载任务时需要展示的摘要。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct DownloadJobRecovery {
    pub job: DownloadJob,
    pub cache_state: DownloadCacheState,
    pub can_install: bool,
    pub can_retry: bool,
    pub user_message: String,
}
```

Add to `src/types.ts`:

```typescript
export type DownloadCacheState = "missing" | "present" | "verified" | "corrupted";

export type DownloadJobRecovery = {
  job: DownloadJob;
  cache_state: DownloadCacheState;
  can_install: boolean;
  can_retry: boolean;
  user_message: string;
};
```

- [ ] **Step 4: 先补恢复判断纯函数失败测试，再实现下载缓存恢复逻辑**

Add to `src-tauri/src/test/download.rs`:

```rust
#[test]
fn recovery_marks_missing_cache_as_retryable() {
    let job = crate::models::DownloadJob {
        id: "download-1".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url: "https://example.invalid/qmclient.zip".to_string(),
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        size: 42,
        status: crate::models::DownloadJobStatus::Verified,
        downloaded_bytes: 42,
        cache_path: "C:/tmp/missing.zip".to_string(),
        error: None,
    };

    let recovery = crate::download::build_download_job_recovery(&job)
        .expect("恢复判断应返回摘要");

    assert_eq!(recovery.cache_state, crate::models::DownloadCacheState::Missing);
    assert_eq!(recovery.can_install, false);
    assert_eq!(recovery.can_retry, true);
}
```

Implement in `src-tauri/src/download.rs`:

```rust
/// 根据下载任务和缓存文件状态构造恢复摘要。
pub fn build_download_job_recovery(job: &DownloadJob) -> Result<DownloadJobRecovery, String> { ... }
```

Recovery rules:

- `Verified` + 文件存在 + 校验通过 → `Verified` / `can_install = true`
- 文件不存在 → `Missing` / `can_retry = true`
- 文件存在但校验失败 → `Corrupted` / `can_retry = true`
- 应用退出前仍处于 `Pending` / `Downloading` / `Installing` → 统一映射成不可继续的恢复态，并给出“请重新下载”提示

- [ ] **Step 5: 先补注册表失败测试，再增加下载任务台账 schema/API**

Add to `src-tauri/src/test/registry.rs`:

```rust
#[test]
fn registry_persists_download_jobs() {
    let temp_dir = tempfile::tempdir().expect("测试目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let job = crate::models::DownloadJob {
        id: "download-1".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url: "https://example.invalid/qmclient.zip".to_string(),
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        size: 42,
        status: crate::models::DownloadJobStatus::Downloading,
        downloaded_bytes: 7,
        cache_path: temp_dir.path().join("download-1.zip").to_string_lossy().replace('\\', "/"),
        error: None,
    };

    registry.upsert_download_job(&job).expect("下载任务应保存成功");
    let jobs = registry.list_download_jobs().expect("下载任务应读取成功");

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "download-1");
    assert_eq!(jobs[0].downloaded_bytes, 7);
}
```

Add to `src-tauri/src/registry.rs`:

```rust
/// 保存或更新一个下载任务快照。
pub fn upsert_download_job(&self, job: &DownloadJob) -> Result<(), String> { ... }

/// 读取所有下载任务快照，按最近更新时间倒序返回。
pub fn list_download_jobs(&self) -> Result<Vec<DownloadJob>, String> { ... }

/// 删除一个下载任务快照，不负责删除缓存文件。
pub fn remove_download_job(&self, job_id: &str) -> Result<(), String> { ... }
```

Schema requirements:

```sql
CREATE TABLE IF NOT EXISTS download_jobs (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  client_installation_id TEXT NOT NULL,
  updated_at TEXT,
  job_json TEXT NOT NULL
)
```

Implementation requirements:

- 使用 `serde_json::to_string(job)` 持久化完整快照
- 使用 `ON CONFLICT(id) DO UPDATE`
- 读取时反序列化 `job_json`
- 不在 registry 层删除缓存文件，文件删除留给 command 层决定

- [ ] **Step 6: 增加恢复查询 / 清理 IPC，并把下载生命周期写穿到注册表**

Add to `src-tauri/src/commands.rs`:

```rust
/// 返回当前可恢复的下载任务摘要。
#[tauri::command]
pub fn list_download_jobs(app: AppHandle) -> Result<Vec<DownloadJobRecovery>, String> { ... }

/// 删除下载任务台账，并按需清理缓存文件。
#[tauri::command]
pub fn remove_download_job(app: AppHandle, job_id: String, delete_cache: bool) -> Result<(), String> { ... }
```

Write-through requirements:

```rust
registry_for_app(&app)?.upsert_download_job(&job)?;
```

Call the write-through helper at these points:

- `start_update_download` 创建任务后
- 下载进度更新后
- 下载校验成功后
- 下载失败后
- 用户取消下载后
- 安装完成后（保留最终状态，供历史区和恢复区判断）

Register in `src-tauri/src/main.rs`:

```rust
commands::list_download_jobs,
commands::remove_download_job,
```

Add wrappers to `src/lib/tauri.ts`:

```typescript
export function listDownloadJobs(): Promise<DownloadJobRecovery[]> {
  return invoke<DownloadJobRecovery[]>("list_download_jobs");
}

export function removeDownloadJob(jobId: string, deleteCache: boolean): Promise<void> {
  return invoke<void>("remove_download_job", { jobId, deleteCache });
}
```

- [ ] **Step 7: 补命令层纯函数测试并跑通检查**

Add to `src-tauri/src/test/commands.rs` a pure helper test for filtering recoverable jobs, for example:

```rust
#[test]
fn list_download_jobs_filters_out_completed_jobs_without_cache() {
    let jobs = vec![
        crate::models::DownloadJob {
            id: "completed-1".to_string(),
            client_installation_id: "qmclient-main".to_string(),
            client_id: "qmclient".to_string(),
            channel: "stable".to_string(),
            version: "2.62.4".to_string(),
            asset_url: "https://example.invalid/qmclient.zip".to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            size: 42,
            status: crate::models::DownloadJobStatus::Completed,
            downloaded_bytes: 42,
            cache_path: "C:/tmp/missing.zip".to_string(),
            error: None,
        },
    ];

    let visible = crate::commands::filter_visible_download_jobs(jobs);
    assert!(visible.is_empty());
}
```

Run:

```powershell
make rust-test
make check
```

Expected: PASS。

- [ ] **Step 8: 提交本任务**

```powershell
git add src-tauri/src/models.rs src-tauri/src/download.rs src-tauri/src/registry.rs src-tauri/src/commands.rs src-tauri/src/main.rs src-tauri/src/test/models.rs src-tauri/src/test/download.rs src-tauri/src/test/registry.rs src-tauri/src/test/commands.rs src/types.ts src/lib/tauri.ts
git commit -m "feat(download): 持久化下载任务恢复台账"
```

**Acceptance:**

- 应用重启后，前端能读取到此前的下载任务摘要。
- 校验通过的缓存能显示“继续安装”。
- 丢失或损坏缓存能显示“重新下载”。
- 中断中的下载不会被误判为可继续安装。

**Risk:**

- `DownloadManager` 内存态与 SQLite 台账可能不一致。
- 处理方式：把“每次状态迁移后立即 upsert”作为唯一写入策略，不做延迟批量落库。

## Task 2: Close the Install Event Feedback Loop

**Goal:** 安装阶段的状态变化不再停留在后端事件；用户点击“安装更新”后，更新页会实时看到安装中、成功、失败三种反馈。

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/components/update/UpdatePanel.tsx`

- [ ] **Step 1: 统一 `install-progress` payload 形状**

Change `src-tauri/src/commands.rs` so `install-progress` emits the full `DownloadJob` snapshot instead of only `job_id`:

```rust
let job = context
    .manager
    .update(context.job_id, |job| {
        job.status = DownloadJobStatus::Installing;
    })?;

context
    .app
    .emit_to("main", "install-progress", &job)
    .map_err(|error| format!("failed to emit install-progress: {error}"))?;
```

This keeps the three install events aligned:

- `install-progress` → `DownloadJob`
- `install-completed` → `DownloadJob`
- `install-failed` → `DownloadJob`

- [ ] **Step 2: 在更新页补安装事件监听**

Update `src/components/update/UpdatePanel.tsx` to subscribe to all install events with the same state sink:

```tsx
void listen<DownloadJob>("install-progress", (event) => {
  setJob(event.payload);
  setError(null);
}).then((fn) => {
  cleanupInstallProgress = fn;
});

void listen<DownloadJob>("install-completed", (event) => {
  setJob(event.payload);
  setError(null);
}).then((fn) => {
  cleanupInstallCompleted = fn;
});

void listen<DownloadJob>("install-failed", (event) => {
  setJob(event.payload);
  setError(event.payload.error ? errorText(event.payload.error) : "安装失败，请稍后重试。");
}).then((fn) => {
  cleanupInstallFailed = fn;
});
```

Remember to dispose all listeners in the cleanup function.

- [ ] **Step 3: 把安装阶段 CTA 和文案收拢到一个派生状态函数**

Add a local helper in `UpdatePanel.tsx`:

```tsx
function primaryActionLabel(job: DownloadJob | null) {
  if (!job) return "开始下载";
  if (job.status === "verified") return "安装更新";
  if (job.status === "installing") return "安装中...";
  if (job.status === "completed") return "已安装完成";
  if (job.status === "failed") return "重新下载";
  return "处理中";
}
```

And update button disabling rules so installation progress cannot be double-triggered:

```tsx
disabled={isBusy || job?.status === "installing" || job?.status === "completed"}
```

- [ ] **Step 4: 运行基础检查，再做一次手工安装链路验证**

Run:

```powershell
make check
```

Then run:

```powershell
make tauri-dev
```

Manual verification:

1. 在更新页完成一次下载。
2. 点击“安装更新”。
3. UI 先进入“安装中”，完成后变为“已安装”；若失败，显示错误文案。
4. 页面刷新或重新进入更新视图时，不应残留未清理的监听器。

- [ ] **Step 5: 提交本任务**

```powershell
git add src-tauri/src/commands.rs src/components/update/UpdatePanel.tsx
git commit -m "feat(update): 接通安装事件前端反馈闭环"
```

**Acceptance:**

- 点击安装后，不需要猜测后端是否在工作。
- 安装成功 / 失败都能被页面即时反映。
- `UpdatePanel` 不再只覆盖“下载”阶段。

**Risk:**

- 安装事件监听与下载事件监听并存，容易造成 cleanup 遗漏。
- 处理方式：所有 `listen()` 返回的 `UnlistenFn` 都单独命名并统一释放。

## Task 3: Surface Recovery, Install History, and Rollback Paths in UpdatePanel

**Goal:** 把“历史记录”和“恢复入口”放回同一个更新面板，让用户知道上次装了什么、失败在哪里、旧版本备份目录在哪、现在应该继续安装还是重新下载。

**Files:**
- Modify: `src/components/update/UpdatePanel.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/types.ts`

- [ ] **Step 1: 在更新页增加恢复区与历史区本地状态**

Add local state to `src/components/update/UpdatePanel.tsx`:

```tsx
const [recoveries, setRecoveries] = useState<DownloadJobRecovery[]>([]);
const [history, setHistory] = useState<InstallHistoryRecord[]>([]);
const [isHistoryLoading, setIsHistoryLoading] = useState(false);
const [historyError, setHistoryError] = useState<string | null>(null);
```

Ensure imports are updated:

```tsx
import type { DownloadJob, DownloadJobRecovery, InstallHistoryRecord } from "../../types";
import { listDownloadJobs, listInstallHistory, removeDownloadJob } from "../../lib/tauri";
```

- [ ] **Step 2: 在默认客户端就绪后并行加载恢复摘要和安装历史**

Add a loader in `UpdatePanel.tsx`:

```tsx
const loadRecoveryAndHistory = async (clientId: string) => {
  setIsHistoryLoading(true);
  setHistoryError(null);
  try {
    const [downloadJobs, installHistory] = await Promise.all([
      listDownloadJobs(),
      listInstallHistory(clientId)
    ]);
    setRecoveries(downloadJobs.filter((item) => item.job.client_installation_id === clientId));
    setHistory(installHistory);
  } catch (error) {
    setHistoryError(errorText(error));
  } finally {
    setIsHistoryLoading(false);
  }
};
```

Call it after the default client is resolved.

- [ ] **Step 3: 渲染“恢复任务”区块，并把动作限制在最小闭环**

Render a recovery section similar to:

```tsx
{recoveries.length > 0 ? (
  <section className="mt-4 rounded-[18px] border border-white/10 bg-black/20 p-4">
    <div className="text-xs font-black uppercase tracking-[0.28em] text-[#41f2ff]/72">
      未完成任务恢复
    </div>
    <div className="mt-3 space-y-3">
      {recoveries.map((item) => (
        <div key={item.job.id} className="rounded-[14px] border border-white/8 bg-white/6 p-3">
          <div className="flex items-center justify-between">
            <span className="text-sm font-black text-white">{item.job.version}</span>
            <span className="text-[11px] font-black text-[#8b93a3]">{item.user_message}</span>
          </div>
          <div className="mt-3 flex gap-2">
            <button
              disabled={!item.can_install || isBusy}
              onClick={() => void installRecoveredJob(item.job.id)}
            >
              继续安装
            </button>
            <button
              disabled={!item.can_retry || isBusy}
              onClick={() => void retryRecoveredJob(item)}
            >
              重新下载
            </button>
            <button
              onClick={() => void removeDownloadJob(item.job.id, false)}
            >
              清理记录
            </button>
          </div>
        </div>
      ))}
    </div>
  </section>
) : null}
```

Interaction rules:

- `can_install` → 允许直接调用 `installDownloadedUpdate(job.id)`
- `can_retry` → 清理当前 `job` 状态并重新走检查更新/下载
- “清理记录”只删台账，默认不删缓存文件；只有后续产品确认后才暴露“同时删除缓存”

- [ ] **Step 4: 渲染安装历史与回滚路径，不新增后端回滚命令**

Render a history section in `UpdatePanel.tsx`:

```tsx
{history.length > 0 ? (
  <section className="mt-4 rounded-[18px] border border-white/10 bg-black/20 p-4">
    <div className="text-xs font-black uppercase tracking-[0.28em] text-[#41f2ff]/72">
      安装历史
    </div>
    <div className="mt-3 space-y-3">
      {history.map((record) => (
        <div key={record.id} className="rounded-[14px] border border-white/8 bg-white/6 p-3">
          <div className="flex items-center justify-between">
            <span className="text-sm font-black text-white">{record.version}</span>
            <span className="text-[11px] font-black text-[#8b93a3]">{record.status === "completed" ? "已完成" : "失败"}</span>
          </div>
          {record.error ? <p className="mt-2 text-xs text-[#ff8f8f]">{record.error}</p> : null}
          {record.rollback_path ? (
            <button
              className="mt-2 text-xs font-black text-[#41f2ff]"
              onClick={() => void navigator.clipboard.writeText(record.rollback_path)}
            >
              复制回滚目录
            </button>
          ) : null}
        </div>
      ))}
    </div>
  </section>
) : null}
```

Important: 当前阶段只展示 / 复制回滚路径，不新增“一键回滚”后端能力。

- [ ] **Step 5: 运行检查并做手工恢复场景验证**

Run:

```powershell
make check
```

Manual verification:

1. 完成一次成功安装，历史区出现成功记录。
2. 人为制造一次安装失败，历史区出现失败记录和错误信息。
3. 如果失败记录存在 `rollback_path`，用户可以复制该路径。
4. 重启应用后，恢复区能看到未完成或可恢复任务。

- [ ] **Step 6: 提交本任务**

```powershell
git add src/components/update/UpdatePanel.tsx src/lib/tauri.ts src/types.ts
git commit -m "feat(update): 展示恢复任务与安装历史"
```

**Acceptance:**

- 更新页成为下载、安装、历史、恢复的统一入口。
- 用户能看到失败原因和回滚目录，而不是只看到“操作失败”。
- 用户重启应用后仍知道自己上次做到哪一步。

**Risk:**

- 恢复区和当前下载卡片可能争抢同一 `job` 状态。
- 处理方式：只允许一个“当前主任务”驱动顶部进度卡，恢复区保留列表态 CTA，不直接复用顶部卡的渲染分支。

## Task 4: Add Real Tauri End-to-End Smoke Validation

**Goal:** 提供一个可重复执行的真实 Tauri smoke 场景，验证“检查更新 → 下载 → 安装 → 重启恢复 → 历史/回滚显示”的整条链路，而不是只停留在单元测试。

**Files:**
- Modify: `Makefile`
- Create: `scripts/tauri_update_smoke.ps1`
- Create: `src-tauri/src/test/fixtures/update-smoke/manifest.json`
- Create: `src-tauri/src/test/fixtures/update-smoke/package-root/`

- [ ] **Step 1: 准备最小 smoke fixture 目录**

Create `src-tauri/src/test/fixtures/update-smoke/manifest.json` with a local asset URL placeholder:

```json
{
  "schema_version": 1,
  "clients": [
    {
      "client_id": "qmclient",
      "channel": "stable",
      "version": "9.9.9-smoke",
      "release_notes": "local smoke build",
      "assets": [
        {
          "platform": "windows-x86_64",
          "asset_url": "http://127.0.0.1:18765/qmclient-smoke.zip",
          "sha256": "__GENERATED_BY_SCRIPT__",
          "size": 0
        }
      ]
    }
  ]
}
```

Create `src-tauri/src/test/fixtures/update-smoke/package-root/` with the minimal client layout required by current `validate_client_dir` rules. Use the same file pattern already proven by existing Rust fixtures; do not invent a different directory shape.

- [ ] **Step 2: 编写 PowerShell smoke 脚本，负责生成 zip、启动本地静态服务和拉起 Tauri**

Create `scripts/tauri_update_smoke.ps1` with responsibilities:

```powershell
# 1. 复制 fixture 到 tmp/tauri-update-smoke
# 2. 从 package-root 生成 qmclient-smoke.zip
# 3. 计算 sha256 / size 并回填 manifest.json
# 4. 用 HttpListener 在 127.0.0.1:18765 提供 manifest 和 zip
# 5. 输出人工验收步骤
# 6. 启动 bun run tauri dev
```

Suggested script entry shape:

```powershell
param()

$root = Join-Path $PSScriptRoot ".."
$smokeRoot = Join-Path $root "tmp/tauri-update-smoke"
$fixtureRoot = Join-Path $root "src-tauri/src/test/fixtures/update-smoke"

# prepare fixture, serve files, then launch tauri dev
```

Do not require Python/Node global binaries; use PowerShell built-ins plus repo-local `bun` / `make` entrypoints.

- [ ] **Step 3: 在 `Makefile` 增加统一入口**

Add:

```make
tauri-smoke-update:
	powershell.exe -ExecutionPolicy Bypass -File scripts/tauri_update_smoke.ps1
```

This keeps smoke validation inside the repository’s `make` contract.

- [ ] **Step 4: 定义并执行一次完整的 smoke 验收**

Run:

```powershell
make tauri-smoke-update
```

Manual smoke checklist:

1. 在应用内把 fixture 客户端设为默认客户端。
2. 在设置里填入本地 smoke manifest URL。
3. 进入更新页并执行“检查更新”。
4. 触发下载，确认看到下载进度。
5. 下载完成后关闭应用并重新打开，确认恢复区出现“继续安装”或等价恢复提示。
6. 完成安装，确认看到安装中 → 已安装。
7. 历史区出现新记录；若制造失败包，历史区应出现失败记录和回滚路径。

- [ ] **Step 5: 运行完整门禁并记录 smoke 结论**

Run:

```powershell
make check-lint
```

Expected:

- `make check-lint` PASS
- smoke 场景至少完成一次成功路径
- 如果失败路径也验证了，需要记录失败原因是否被正确展示，而不是简单“跑过就算”

- [ ] **Step 6: 提交本任务**

```powershell
git add Makefile scripts/tauri_update_smoke.ps1 src-tauri/src/test/fixtures/update-smoke
git commit -m "test(tauri): 增加更新链路 smoke 验收入口"
```

**Acceptance:**

- 仓库内有可重复执行的真实 Tauri 更新 smoke 命令。
- smoke 不依赖额外全局工具。
- 能覆盖至少一次“下载完成后重启恢复”和一次“安装历史写回 UI”的真实链路。

**Risk:**

- Windows 文件锁、杀软或本地权限会让 smoke 偶发失败。
- 处理方式：fixture、缓存目录、安装目录全部放在 `tmp/tauri-update-smoke` 下，避免污染真实用户环境；失败时保留日志和目录便于复现。

## Final Verification Matrix

- [ ] `make rust-test`
- [ ] `make check`
- [ ] `make check-lint`
- [ ] `make tauri-smoke-update`
- [ ] 手工验证安装中 / 安装完成 / 安装失败三类 UI 反馈
- [ ] 手工验证恢复区在应用重启后仍可见
- [ ] 手工验证安装历史与回滚路径展示

## Implementation Notes for the Engineer

- `src/App.tsx` 当前已有 `update` 视图，默认不要新增页面或导航。
- `src/lib/tauri.ts` 是唯一允许新增前端 Tauri 调用封装的位置；组件内不要散落裸 `invoke`。
- 不要在本计划里顺带引入结构化错误码大改；优先完成用户可感知闭环。
- `remove_download_job(..., delete_cache)` 后端参数先保留，但 UI 默认只暴露“清理记录”而不暴露“删缓存”，避免误删用户文件。
- 如果实现阶段改动了 `Makefile`、`src-tauri/src/commands.rs`、`src/lib/tauri.ts` 这些核心边界，完成后按仓库规则补一次只读审查子代理。
