import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScanProgressEvent } from "../lib/scanProgress";
import { useClientScanner } from "./useClientScanner";

let emitHandler: ((e: { payload: ScanProgressEvent }) => void) | null = null;
const unlisten = vi.fn();
const invoke = vi.fn();
const listen = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args)
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listen(...args)
}));

describe("useClientScanner", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    emitHandler = null;
    listen.mockImplementation(async (_event: string, handler: (e: { payload: ScanProgressEvent }) => void) => {
      emitHandler = handler;
      return unlisten;
    });
    invoke.mockResolvedValue([]);
  });

  it("subscribes scan-progress on mount and unsubscribes on unmount", async () => {
    const { unmount } = renderHook(() => useClientScanner());

    await waitFor(() => {
      expect(listen).toHaveBeenCalledWith("scan-progress", expect.any(Function));
    });

    unmount();
    expect(unlisten).toHaveBeenCalled();
  });

  it("appends events and updates foundCount when entries_found arrives", async () => {
    const { result } = renderHook(() => useClientScanner());

    await waitFor(() => {
      expect(emitHandler).not.toBeNull();
    });

    act(() => {
      emitHandler!({ payload: { kind: "drive_started", root: "C:\\", backend: "mft" } });
    });
    act(() => {
      emitHandler!({ payload: { kind: "entries_found", found: 7 } });
    });

    expect(result.current.events).toHaveLength(2);
    expect(result.current.foundCount).toBe(7);
  });

  it("appends drive_completed event without updating foundCount (parallel scan)", async () => {
    // 多盘并行下，DriveCompleted.found 是 per-drive 语义，不再用于覆盖 foundCount。
    // foundCount 全权由 entries_found（后端 GlobalizingSink 已转全局累计）驱动。
    const { result } = renderHook(() => useClientScanner());

    await waitFor(() => {
      expect(emitHandler).not.toBeNull();
    });

    act(() => {
      emitHandler!({ payload: { kind: "drive_completed", root: "C:\\", scanned: 100, found: 3 } });
    });

    expect(result.current.events).toHaveLength(1);
    expect(result.current.foundCount).toBe(0);
  });

  it("start invokes scan_clients_via_mft, clears events, and returns installations", async () => {
    const fakeClient = { id: "qmclient-abc", display_name: "QmClient" };
    invoke.mockResolvedValueOnce([fakeClient]);

    const { result } = renderHook(() => useClientScanner());

    await waitFor(() => expect(emitHandler).not.toBeNull());

    act(() => {
      emitHandler!({ payload: { kind: "entries_found", found: 1 } });
    });
    expect(result.current.events).toHaveLength(1);

    let installations: unknown[] = [];
    await act(async () => {
      installations = await result.current.start();
    });

    expect(invoke).toHaveBeenCalledWith("scan_clients_via_mft", { options: null });
    expect(installations).toEqual([fakeClient]);
    expect(result.current.events).toHaveLength(0);
    expect(result.current.scanning).toBe(false);
  });

  it("captures invoke error into error state and rethrows", async () => {
    invoke.mockRejectedValueOnce(new Error("boom"));

    const { result } = renderHook(() => useClientScanner());

    await act(async () => {
      await expect(result.current.start()).rejects.toThrow("boom");
    });

    expect(result.current.error).toBe("boom");
    expect(result.current.scanning).toBe(false);
  });

  it("cancel invokes cancel_scan_clients and returns true on success", async () => {
    invoke.mockResolvedValueOnce(true);
    const { result } = renderHook(() => useClientScanner());

    let cancelled = false;
    await act(async () => {
      cancelled = await result.current.cancel();
    });

    expect(cancelled).toBe(true);
    expect(invoke).toHaveBeenCalledWith("cancel_scan_clients");
  });

  it("cancel returns false and sets error when invoke rejects", async () => {
    invoke.mockRejectedValueOnce(new Error("network down"));
    const { result } = renderHook(() => useClientScanner());

    let cancelled = true;
    await act(async () => {
      cancelled = await result.current.cancel();
    });

    expect(cancelled).toBe(false);
    expect(result.current.error).toContain("network down");
  });

  it("reset clears events, foundCount, and error", async () => {
    const { result } = renderHook(() => useClientScanner());

    await waitFor(() => expect(emitHandler).not.toBeNull());

    act(() => {
      emitHandler!({ payload: { kind: "entries_found", found: 5 } });
    });
    act(() => {
      result.current.reset();
    });

    expect(result.current.events).toHaveLength(0);
    expect(result.current.foundCount).toBe(0);
    expect(result.current.error).toBeNull();
  });
});
