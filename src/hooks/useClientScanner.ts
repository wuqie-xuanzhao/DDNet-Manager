import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ClientInstallation, ScanClientInstallationsOptions } from "../types";
import type { ScanProgressEvent } from "../lib/scanProgress";

export interface ScanClientsViaMftParams {
  options?: ScanClientInstallationsOptions;
}

/// events 数组软上限。长时间全盘扫描会产生数百条事件（drive_started /
/// entries_found / drive_completed 等），UI 时间线只看尾部即可，cap 后避免
/// 内存增长 + setState 触发的 re-render 成本随事件数线性增加。
const MAX_EVENTS = 50;

export interface UseClientScannerResult {
  /** 最近一条进度事件（按 root 聚合的 Map） */
  events: ScanProgressEvent[];
  /** 累计找到的客户端数（来自最后一条 entries_found / drive_completed） */
  foundCount: number;
  /** 是否正在扫描 */
  scanning: boolean;
  /** 错误（invoke 失败或事件错误） */
  error: string | null;
  /** 触发扫描 */
  start: (params?: ScanClientsViaMftParams) => Promise<ClientInstallation[]>;
  /** 取消当前扫描（调 cancel_scan_clients command） */
  cancel: () => Promise<boolean>;
  /** 手动清空事件流 */
  reset: () => void;
}

/**
 * 监听 `scan-progress` 事件 + 调用 `scan_clients_via_mft` command。
 *
 * ntfs-search crate 在 Rust 端 emit 事件，hook 自动收集到 `events`，
 * UI 用 `events.map(describeScanEvent)` 渲染时间线。
 */
export function useClientScanner(): UseClientScannerResult {
  const [events, setEvents] = useState<ScanProgressEvent[]>([]);
  const [foundCount, setFoundCount] = useState(0);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 监听扫描进度事件——hook 内部订阅，组件级用 useClientScanner 即可
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<ScanProgressEvent>("scan-progress", (e) => {
      if (cancelled) return;
      const event = e.payload;
      setEvents((prev) => {
        const next = [...prev, event];
        // cap 到最近 MAX_EVENTS 条：UI 时间线只渲染尾部，过长无意义且拖慢 re-render
        return next.length > MAX_EVENTS ? next.slice(next.length - MAX_EVENTS) : next;
      });
      // 仅由 entries_found 驱动 foundCount。多盘并行下，后端 GlobalizingSink 把
      // 各盘 per-drive found 累加成全局总数后 emit，覆盖式更新即正确。
      // 不再用 drive_completed.found（per-drive 语义），否则并行下会跳变回退。
      if (event.kind === "entries_found") {
        setFoundCount(event.found);
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(`订阅 scan-progress 失败：${String(err)}`);
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const start = useCallback(
    async (params?: ScanClientsViaMftParams): Promise<ClientInstallation[]> => {
      setScanning(true);
      setError(null);
      setEvents([]);
      setFoundCount(0);
      try {
        const installations = await invoke<ClientInstallation[]>(
          "scan_clients_via_mft",
          { options: params?.options ?? null }
        );
        return installations;
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        throw err;
      } finally {
        setScanning(false);
      }
    },
    []
  );

  const reset = useCallback(() => {
    setEvents([]);
    setFoundCount(0);
    setError(null);
  }, []);

  const cancel = useCallback(async (): Promise<boolean> => {
    try {
      const cancelled = await invoke<boolean>("cancel_scan_clients");
      if (cancelled) {
        // 后端确认取消，前端立即停止"扫描中"显示；实际 scan_clients_via_mft
        // 会在 master_cancel 触发后秒级返回，UI 状态对齐避免一直 spinner
        setScanning(false);
      }
      return cancelled;
    } catch (err) {
      setError(`取消扫描失败：${err instanceof Error ? err.message : String(err)}`);
      return false;
    }
  }, []);

  return { events, foundCount, scanning, error, start, cancel, reset };
}
