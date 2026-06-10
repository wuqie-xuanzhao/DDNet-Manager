import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../lib/settings";
import { useClientLauncher } from "./useClientLauncher";

const getLaunchReadiness = vi.fn();
const launchDefaultClient = vi.fn();
const reportLocalSmokeResult = vi.fn();
const upsertClientInstallation = vi.fn();
const validateClientDir = vi.fn();
const closeWindow = vi.fn();
const minimizeWindow = vi.fn();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: closeWindow,
    minimize: minimizeWindow
  })
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));

vi.mock("../lib/tauri", () => ({
  getLaunchReadiness: (...args: unknown[]) => getLaunchReadiness(...args),
  launchDefaultClient: (...args: unknown[]) => launchDefaultClient(...args),
  reportLocalSmokeResult: (...args: unknown[]) => reportLocalSmokeResult(...args),
  upsertClientInstallation: (...args: unknown[]) => upsertClientInstallation(...args),
  validateClientDir: (...args: unknown[]) => validateClientDir(...args)
}));

const readyClient = {
  id: "qmclient-default",
  client_id: "qmclient",
  display_name: "QmClient",
  install_dir: "D:/Games/QmClient",
  executable_path: "D:/Games/QmClient/DDNet.exe",
  storage_cfg_path: "D:/Games/QmClient/storage.cfg",
  data_dir: "D:/Games/QmClient/data",
  user_data_dir: null,
  version: null,
  is_default: true,
  health: "ok",
  missing_items: [],
  install_source: "manual",
  confidence: "verified",
  manager_owned: false,
  compatibility: {
    status: "supported",
    can_launch: true,
    launch_verified: false,
    last_launch_result: null,
    reasons: []
  },
  upstream_url: null,
  last_scanned_at: "2026-06-08T00:00:00Z"
} as const;

describe("useClientLauncher local smoke bootstrap", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    closeWindow.mockResolvedValue(undefined);
    minimizeWindow.mockResolvedValue(undefined);
    launchDefaultClient.mockResolvedValue(undefined);
    reportLocalSmokeResult.mockResolvedValue(undefined);
    getLaunchReadiness.mockResolvedValue({
      client: null,
      can_launch: false,
      running: false,
      status_label: "未设置",
      user_message: "尚未设置默认客户端。",
      blocking_reasons: ["没有默认客户端记录"],
      checked_at: "2026-06-08T00:00:00.000Z"
    });
  });

  it("reports a failed smoke result when bootstrap validation fails", async () => {
    validateClientDir.mockRejectedValue(new Error("invalid client dir"));

    renderHook(() =>
      useClientLauncher({
        appSettings: defaultAppSettings,
        localSmokeAutomation: {
          clientInstallDir: "D:/Missing/QmClient",
          manifestUrl: "http://127.0.0.1:18765/manifest.json",
          closeWindowOnFinish: false
        },
        onOpenUpdateView: vi.fn(),
        tauriRuntime: true
      })
    );

    await waitFor(() => {
      expect(reportLocalSmokeResult).toHaveBeenCalledWith({
        status: "failed",
        stage: "bootstrap",
        message: expect.stringContaining("验证失败")
      });
    });
  });

  it("closes the smoke window after bootstrap validation reports failure", async () => {
    validateClientDir.mockRejectedValue(new Error("invalid client dir"));

    renderHook(() =>
      useClientLauncher({
        appSettings: defaultAppSettings,
        localSmokeAutomation: {
          clientInstallDir: "D:/Missing/QmClient",
          manifestUrl: "http://127.0.0.1:18765/manifest.json",
          closeWindowOnFinish: true
        },
        onOpenUpdateView: vi.fn(),
        tauriRuntime: true
      })
    );

    await waitFor(() => {
      expect(reportLocalSmokeResult).toHaveBeenCalledWith({
        status: "failed",
        stage: "bootstrap",
        message: expect.stringContaining("验证失败")
      });
    });
    await waitFor(() => {
      expect(closeWindow).toHaveBeenCalledTimes(1);
    });
  });

  it("persists the local smoke client without making it the default client during bootstrap", async () => {
    const smokeClient = {
      ...readyClient,
      id: "qmclient-smoke",
      install_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
      executable_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/DDNet.exe",
      storage_cfg_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/storage.cfg",
      data_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/data",
      is_default: false,
      last_scanned_at: "2026-06-08T00:00:00Z"
    };
    validateClientDir.mockResolvedValue(smokeClient);
    upsertClientInstallation.mockResolvedValue(smokeClient);

    renderHook(() =>
      useClientLauncher({
        appSettings: defaultAppSettings,
        localSmokeAutomation: {
          clientInstallDir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
          manifestUrl: "http://127.0.0.1:18765/manifest.json",
          closeWindowOnFinish: false
        },
        onOpenUpdateView: vi.fn(),
        tauriRuntime: true
      })
    );

    await waitFor(() => {
      expect(upsertClientInstallation).toHaveBeenCalledWith({
        install_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
        is_default: false
      });
    });
  });

  it("minimizes the manager after the launch command succeeds even when readiness refresh fails", async () => {
    getLaunchReadiness.mockResolvedValueOnce({
      client: readyClient,
      can_launch: true,
      running: false,
      status_label: "可启动",
      user_message: "QmClient 已准备就绪，可以启动。",
      blocking_reasons: [],
      checked_at: "2026-06-08T00:00:00.000Z"
    });
    getLaunchReadiness.mockRejectedValueOnce(new Error("process probe failed"));

    const { result } = renderHook(() =>
      useClientLauncher({
        appSettings: {
          ...defaultAppSettings,
          close_panel_after_launch: true
        },
        localSmokeAutomation: null,
        onOpenUpdateView: vi.fn(),
        tauriRuntime: true
      })
    );

    await waitFor(() => {
      expect(result.current.launcherState).toBe("ready");
    });

    await result.current.handlePrimaryAction();

    expect(launchDefaultClient).toHaveBeenCalledTimes(1);
    expect(minimizeWindow).toHaveBeenCalledTimes(1);
  });

  it("keeps the launcher running state when window minimize fails after launch succeeds", async () => {
    getLaunchReadiness.mockResolvedValueOnce({
      client: readyClient,
      can_launch: true,
      running: false,
      status_label: "可启动",
      user_message: "QmClient 已准备就绪，可以启动。",
      blocking_reasons: [],
      checked_at: "2026-06-08T00:00:00.000Z"
    });
    getLaunchReadiness.mockResolvedValueOnce({
      client: readyClient,
      can_launch: true,
      running: true,
      status_label: "正在运行",
      user_message: "QmClient 正在运行。",
      blocking_reasons: [],
      checked_at: "2026-06-08T00:00:01.000Z"
    });
    minimizeWindow.mockRejectedValueOnce(new Error("window unavailable"));

    const { result } = renderHook(() =>
      useClientLauncher({
        appSettings: {
          ...defaultAppSettings,
          close_panel_after_launch: true
        },
        localSmokeAutomation: null,
        onOpenUpdateView: vi.fn(),
        tauriRuntime: true
      })
    );

    await waitFor(() => {
      expect(result.current.launcherState).toBe("ready");
    });

    await act(async () => {
      await result.current.handlePrimaryAction();
    });

    expect(launchDefaultClient).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(result.current.launcherState).toBe("running");
      expect(result.current.errorMessage).toContain("启动成功，但最小化启动器失败");
    });
  });
});
