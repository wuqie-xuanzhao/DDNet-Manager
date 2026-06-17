/**
 * ntfs-search crate 进度事件，对应 Rust 端 `ntfs_search::ProgressEvent` enum。
 *
 * Rust 端用 `#[serde(tag = "kind", rename_all = "snake_case")]` 序列化，
 * 前端按 `event.kind` 做 discriminated union 处理。
 */

export type BackendKind = "mft" | "usn" | "walkdir";

export type ScanLimitKind = "results" | "records_scanned";

export type ScanProgressEvent =
  | { kind: "drive_started"; root: string; backend: BackendKind }
  | { kind: "entries_found"; found: number }
  | {
      kind: "drive_completed";
      root: string;
      scanned: number;
      found: number;
    }
  | {
      kind: "backend_downgraded";
      root: string;
      from: BackendKind;
      to: BackendKind;
      reason: string;
    }
  | { kind: "scan_limit_hit"; limit_kind: ScanLimitKind; limit: number }
  | { kind: "drive_skipped"; root: string; reasons: string[] }
  | { kind: "entry_error"; path: string | null; error: string };

/**
 * 把 backend kind 转中文显示标签。
 */
export function describeBackend(backend: BackendKind): string {
  switch (backend) {
    case "mft":
      return "MFT（管理员）";
    case "usn":
      return "USN（普通用户）";
    case "walkdir":
      return "Walkdir（fallback）";
  }
}

/**
 * 把 ScanProgressEvent 转简短中文描述（用于 UI 一行展示）。
 */
export function describeScanEvent(event: ScanProgressEvent): string {
  switch (event.kind) {
    case "drive_started":
      return `扫描 ${event.root}（${describeBackend(event.backend)}）`;
    case "entries_found":
      return `已找到 ${event.found} 个候选`;
    case "drive_completed":
      return `${event.root} 完成：扫描 ${event.scanned} 条记录，找到 ${event.found} 个`;
    case "backend_downgraded":
      return `${event.root} 后端降级 ${describeBackend(event.from)} → ${describeBackend(event.to)}：${event.reason}`;
    case "scan_limit_hit":
      return event.limit_kind === "results"
        ? `命中结果上限 ${event.limit}，提前停止`
        : `命中扫描上限 ${event.limit}，提前停止`;
    case "drive_skipped":
      return `${event.root} 跳过：${event.reasons.join("；")}`;
    case "entry_error":
      return event.path
        ? `条目错误 ${event.path}：${event.error}`
        : `条目错误：${event.error}`;
  }
}
