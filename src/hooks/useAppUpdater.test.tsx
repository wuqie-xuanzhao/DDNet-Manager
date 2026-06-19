import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppUpdateCheck } from "../types";
import { useAppUpdater } from "./useAppUpdater";

const checkAppUpdate = vi.fn();
const checkUpdater = vi.fn();
const relaunch = vi.fn();

vi.mock("../lib/tauri", () => ({
  checkAppUpdate: (...a: unknown[]) => checkAppUpdate(...a)
}));

vi.mock("../lib/errors", () => ({
  getErrorMessage: (e: unknown) => (e instanceof Error ? e.message : String(e))
}));

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...a: unknown[]) => checkUpdater(...a)
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: (...a: unknown[]) => relaunch(...a)
}));

const baseSettings = {
  network_route: null,
  scan_excluded_paths: [],
  scan_max_results: null,
  scan_timeout_secs: null,
  close_panel_after_launch: true,
  auto_check_updates: false,
  autostart: false,
  exit_game_show_launcher: true,
  close_behavior: "ask",
  allow_silent_update: true
};

const hasUpdateResult: AppUpdateCheck = {
  current_version: "0.1.0",
  latest_version: "0.2.0",
  has_update: true,
  release_url: "https://github.com/example/repo/releases/tag/v0.2.0",
  release_notes: "feat: 新功能"
};

const upToDateResult: AppUpdateCheck = {
  current_version: "0.1.0",
  latest_version: "0.1.0",
  has_update: false,
  release_url: "https://github.com/example/repo/releases",
  release_notes: null
};

describe("useAppUpdater", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    checkAppUpdate.mockResolvedValue(upToDateResult);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // 自动检查延迟 1.5s，测试需要等真实时间。每个用例 ~2s。
  // 简化方案：不用 fake timers（fake + async/await 不兼容，会卡 microtask flush）。

  it("allow_silent_update=false 时不自动检查，state 保持 idle，按钮不可见", async () => {
    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: { ...baseSettings, allow_silent_update: false } as never
      })
    );

    // 等 2s 确保自动检查（如果配置错误触发）有时间跑
    await new Promise((r) => setTimeout(r, 2000));
    expect(checkAppUpdate).not.toHaveBeenCalled();
    expect(result.current.state).toBe("idle");
    expect(result.current.visible).toBe(false);
  });

  it("启动后自动检查 + 拉到 has-update → state 切 has-update", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(
      () => {
        expect(result.current.state).toBe("has-update");
      },
      { timeout: 5000 }
    );
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);
    expect(result.current.updateInfo?.latest_version).toBe("0.2.0");
    expect(result.current.visible).toBe(true);
  });

  it("拉到 up-to-date → state 切 up-to-date", async () => {
    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(
      () => {
        expect(result.current.state).toBe("up-to-date");
      },
      { timeout: 5000 }
    );
  });

  it("checkAppUpdate 失败 → state 切 failed + error", async () => {
    checkAppUpdate.mockRejectedValue(new Error("network down"));

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(
      () => {
        expect(result.current.state).toBe("failed");
      },
      { timeout: 5000 }
    );
    expect(result.current.error).toBe("network down");
  });

  it("checkForUpdate 手动触发 + force 跳过冷却", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    // 等自动检查完成
    await waitFor(
      () => expect(result.current.state).toBe("has-update"),
      { timeout: 5000 }
    );
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);

    // 冷却期内手动调用（无 force）不触发
    await act(async () => {
      await result.current.checkForUpdate();
    });
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);

    // force 跳过冷却
    await act(async () => {
      await result.current.checkForUpdate({ force: true });
    });
    expect(checkAppUpdate).toHaveBeenCalledTimes(2);
  });

  it("tauriRuntime=false 时不检查", async () => {
    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: false,
        appSettings: baseSettings as never
      })
    );

    await new Promise((r) => setTimeout(r, 2000));
    expect(checkAppUpdate).not.toHaveBeenCalled();
    expect(result.current.state).toBe("idle");
  });

  // ===== downloadAndInstall 测试（新状态：downloading/installing/ready-to-restart）=====

  /// 构造一个 fake Update，模拟 plugin-updater 的 downloadAndInstall 行为：
  /// 调用 onEvent 触发 Started / Progress / Finished，然后 Promise resolve。
  function makeFakeUpdate(opts?: { failWith?: Error }) {
    return {
      version: "0.2.0",
      currentVersion: "0.1.0",
      downloadAndInstall: vi.fn(async (onEvent: (event: { event: string; data: Record<string, unknown> }) => void) => {
        if (opts?.failWith) throw opts.failWith;
        onEvent({ event: "Started", data: { contentLength: 1000 } });
        onEvent({ event: "Progress", data: { chunkLength: 400 } });
        onEvent({ event: "Progress", data: { chunkLength: 600 } });
        onEvent({ event: "Finished", data: {} });
      }),
      close: vi.fn(async () => {})
    };
  }

  it("downloadAndInstall 流式更新 progress 并最终切到 ready-to-restart", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);
    checkUpdater.mockResolvedValue(makeFakeUpdate());

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(() => expect(result.current.state).toBe("has-update"), { timeout: 5000 });

    await act(async () => {
      await result.current.downloadAndInstall();
    });

    expect(checkUpdater).toHaveBeenCalledTimes(1);
    expect(result.current.state).toBe("ready-to-restart");
    // 流式进度最后一帧应是 1000/1000 = 100%
    expect(result.current.progress).toBeNull(); // 完成后清空
  });

  it("downloadAndInstall 失败时 state 切 failed + installError 填充", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);
    checkUpdater.mockResolvedValue(makeFakeUpdate({ failWith: new Error("签名校验失败") }));

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(() => expect(result.current.state).toBe("has-update"), { timeout: 5000 });

    await act(async () => {
      await result.current.downloadAndInstall();
    });

    expect(result.current.state).toBe("failed");
    expect(result.current.installError).toBe("签名校验失败");
    expect(result.current.progress).toBeNull();
  });

  it("restartNow 调用 process.relaunch()", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);
    checkUpdater.mockResolvedValue(makeFakeUpdate());

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(() => expect(result.current.state).toBe("has-update"), { timeout: 5000 });

    await act(async () => {
      await result.current.downloadAndInstall();
    });

    expect(result.current.state).toBe("ready-to-restart");

    await act(async () => {
      await result.current.restartNow();
    });

    expect(relaunch).toHaveBeenCalledTimes(1);
  });

  // ===== 回归：cancel 守卫 + Progress NaN 防御 =====

  /// 可控进度的 fake Update：测试需要在外面驱动 onEvent + 决定 Promise 何时 resolve。
  /// 用来验证"cancel 后 Promise 回调不再 setState"这一关键回归。
  function makeControllableUpdate() {
    let resolveFn!: () => void;
    let rejectFn!: (e: Error) => void;
    const promise = new Promise<void>((res, rej) => {
      resolveFn = res;
      rejectFn = rej;
    });
    const onEventRef: { current: ((e: { event: string; data: Record<string, unknown> }) => void) | null } = {
      current: null
    };
    return {
      version: "0.2.0",
      currentVersion: "0.1.0",
      downloadAndInstall: vi.fn((onEvent: (e: { event: string; data: Record<string, unknown> }) => void) => {
        onEventRef.current = onEvent;
        return promise;
      }),
      emit(event: { event: string; data: Record<string, unknown> }) {
        onEventRef.current?.(event);
      },
      finish() {
        resolveFn();
      },
      fail(err: Error) {
        rejectFn(err);
      },
      close: vi.fn(async () => {})
    };
  }

  it("cancel 后再 downloadAndInstall 可重新触发", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);
    // 每次调用 checkUpdater 都新建一个 controllable update（mockResolvedValue 会
    // 缓存同一个 Promise，让第二次调用复用已 resolved 的 Promise，造成 state 直接
    // 跳到 ready-to-restart 的假象；mockImplementation 模拟真实 plugin 每次返回新实例）。
    // 用 array 跟踪所有创建的实例，避免 mockImplementation 内部覆盖外部引用。
    const instances: ReturnType<typeof makeControllableUpdate>[] = [];
    checkUpdater.mockImplementation(() => {
      const inst = makeControllableUpdate();
      instances.push(inst);
      return Promise.resolve(inst);
    });

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(() => expect(result.current.state).toBe("has-update"), { timeout: 5000 });

    // 第一次下载
    await act(async () => {
      void result.current.downloadAndInstall();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.state).toBe("downloading"), { timeout: 2000 });
    expect(instances).toHaveLength(1);
    const first = instances[0];
    expect(first.downloadAndInstall).toHaveBeenCalledTimes(1);

    // cancel：UI 立即回到 has-update
    await act(async () => {
      result.current.cancelDownload();
    });
    expect(result.current.state).toBe("has-update");

    // 让后台 Promise resolve 掉（模拟 plugin 完成下载），UI 不应变 ready-to-restart
    first.emit({ event: "Finished", data: {} });
    await act(async () => {
      first.finish();
      await Promise.resolve();
    });
    expect(result.current.state).toBe("has-update");

    // 再次 downloadAndInstall：ref 已被 cancel 清空，应可触发，且创建新实例
    await act(async () => {
      void result.current.downloadAndInstall();
      await Promise.resolve();
    });
    await waitFor(() => expect(instances).toHaveLength(2), { timeout: 2000 });
    await waitFor(() => expect(result.current.state).toBe("downloading"), { timeout: 2000 });
    expect(instances[1].downloadAndInstall).toHaveBeenCalledTimes(1);

    // 清理后台 Promise，避免 unhandled rejection
    await act(async () => {
      result.current.cancelDownload();
      instances[1].finish();
      await Promise.resolve();
    });
  });

  it("cancel 后 Promise resolve 不再把 state 切到 ready-to-restart（回归守卫）", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);
    const ctrl = makeControllableUpdate();
    checkUpdater.mockResolvedValue(ctrl);

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(() => expect(result.current.state).toBe("has-update"), { timeout: 5000 });

    await act(async () => {
      void result.current.downloadAndInstall();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.state).toBe("downloading"), { timeout: 2000 });

    // 中途发 Progress + cancel + 后续事件 + 完成
    ctrl.emit({ event: "Progress", data: { chunkLength: 500 } });
    await act(async () => {
      result.current.cancelDownload();
    });
    expect(result.current.state).toBe("has-update");
    expect(result.current.progress).toBeNull();

    // cancel 后再发事件、再 resolve：UI 必须保持 has-update
    ctrl.emit({ event: "Progress", data: { chunkLength: 500 } });
    ctrl.emit({ event: "Finished", data: {} });
    await act(async () => {
      ctrl.finish();
      await Promise.resolve();
    });

    expect(result.current.state).toBe("has-update");
    expect(result.current.progress).toBeNull();
  });

  it("Progress 回调中 chunkLength/contentLength 缺失或非法时不产生 NaN", async () => {
    checkAppUpdate.mockResolvedValue(hasUpdateResult);
    const ctrl = makeControllableUpdate();
    checkUpdater.mockResolvedValue(ctrl);

    const { result } = renderHook(() =>
      useAppUpdater({
        tauriRuntime: true,
        appSettings: baseSettings as never
      })
    );

    await waitFor(() => expect(result.current.state).toBe("has-update"), { timeout: 5000 });

    await act(async () => {
      void result.current.downloadAndInstall();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.state).toBe("downloading"), { timeout: 2000 });

    // Started 缺 contentLength（模拟 plugin 异常 payload）→ total=0
    ctrl.emit({ event: "Started", data: {} });
    expect(result.current.progress?.total).toBe(0);

    // Progress chunkLength 缺失 / 负数 / 非数字 → 都被 Number() || 0 兜底为 0
    ctrl.emit({ event: "Progress", data: {} });
    ctrl.emit({ event: "Progress", data: { chunkLength: undefined } });
    ctrl.emit({ event: "Progress", data: { chunkLength: -1 } });
    ctrl.emit({ event: "Progress", data: { chunkLength: "not-a-number" as unknown as number } });

    // 任何情况下 ratio 都不能是 NaN / Infinity
    expect(Number.isFinite(result.current.progress?.ratio ?? NaN)).toBe(true);
    expect(result.current.progress?.ratio).toBe(0); // total=0 时 ratio 保持 0（不除零）
    expect(result.current.progress?.downloaded).toBe(0); // 所有非法 chunk 累加为 0

    // 清理后台 Promise
    await act(async () => {
      result.current.cancelDownload();
      ctrl.finish();
      await Promise.resolve();
    });
  });
});
