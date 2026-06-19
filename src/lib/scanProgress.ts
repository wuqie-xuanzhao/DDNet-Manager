/**
 * ntfs-search crate 进度事件，对应 Rust 端 `ntfs_search::ProgressEvent` enum。
 *
 * Rust 端用 `#[serde(tag = "kind", rename_all = "snake_case")]` 序列化，
 * 前端按 `event.kind` 做 discriminated union 处理。
 *
 * 另外业务层（DDNet-Manager scan.rs）会通过同一 `scan-progress` 通道发
 * `phase_started` 变体（`ScanPhaseEvent`），不属于 ntfs-search enum，但共用
 * 联合类型——前端按 `kind` 区分。
 */

export type BackendKind = "mft" | "usn" | "walkdir";

export type ScanLimitKind = "results" | "records_scanned";

/** 业务层扫描阶段标签，对应 Rust 端 `ScanPhase` enum（snake_case 序列化）。 */
export type ScanPhase = "started" | "priority" | "fallback";

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
  | { kind: "entry_error"; path: string | null; error: string }
  /// 业务层扫描阶段事件（DDNet-Manager scan.rs emit，非 ntfs-search）。
  /// 让前端在 ntfs-search 第一条 drive_started 之前就能显示"扫描中"避免黑屏。
  | { kind: "phase_started"; phase: ScanPhase };

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
    case "phase_started":
      return describeScanPhase(event.phase);
  }
}

/**
 * 把扫描阶段标签转中文显示。phase_started 事件用。
 */
export function describeScanPhase(phase: ScanPhase): string {
  switch (phase) {
    case "started":
      return "扫描已启动，正在准备扫描位置…";
    case "priority":
      return "正在扫描常见安装位置（Steam / Program Files / 用户目录）…";
    case "fallback":
      return "未在常见位置命中，扩展到全盘扫描…";
  }
}

/// ScanProgressEvent 数组软上限。useClientScanner 和 useClientInstaller 共用：
/// 长时间全盘扫描会 emit 数百条事件，UI 时间线只看尾部即可，cap 后避免内存增长
/// + setState 触发的 re-render 成本随事件数线性增加。
export const MAX_SCAN_EVENTS = 50;

/**
 * 把事件追加到 prev 尾部，超过 MAX_SCAN_EVENTS 时丢弃头部。
 *
 * useClientScanner 和 useClientInstaller 都监听 scan-progress 累积事件，cap 逻辑
 * 共用一份避免重复（review issue L2）。已达上限时 `slice(prev.length - max + 1)`
 * 比 `[...prev, event].slice(...)` 省一次 array copy（review issue L1）。
 */
export function appendScanEventCapped(
  prev: ScanProgressEvent[],
  event: ScanProgressEvent,
  max: number = MAX_SCAN_EVENTS
): ScanProgressEvent[] {
  if (prev.length >= max) {
    return [...prev.slice(prev.length - max + 1), event];
  }
  return [...prev, event];
}
