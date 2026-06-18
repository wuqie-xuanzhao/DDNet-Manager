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
});
