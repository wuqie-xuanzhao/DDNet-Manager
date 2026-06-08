import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../lib/settings";
import { useClientLauncher } from "./useClientLauncher";

const getLaunchReadiness = vi.fn();
const reportLocalSmokeResult = vi.fn();
const upsertClientInstallation = vi.fn();
const validateClientDir = vi.fn();
const closeWindow = vi.fn();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: closeWindow,
    minimize: vi.fn()
  })
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));

vi.mock("../lib/tauri", () => ({
  getLaunchReadiness: (...args: unknown[]) => getLaunchReadiness(...args),
  launchDefaultClient: vi.fn(),
  reportLocalSmokeResult: (...args: unknown[]) => reportLocalSmokeResult(...args),
  upsertClientInstallation: (...args: unknown[]) => upsertClientInstallation(...args),
  validateClientDir: (...args: unknown[]) => validateClientDir(...args)
}));

describe("useClientLauncher local smoke bootstrap", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    closeWindow.mockResolvedValue(undefined);
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
      id: "qmclient-smoke",
      client_id: "qmclient",
      display_name: "QmClient",
      install_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
      executable_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/DDNet.exe",
      storage_cfg_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/storage.cfg",
      data_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/data",
      user_data_dir: null,
      version: null,
      is_default: false,
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
});
