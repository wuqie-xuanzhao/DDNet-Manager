import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UpdatePanel } from "./UpdatePanel";
import type { ClientInstallation, DownloadJob } from "../../types";
import type * as TauriApi from "../../lib/tauri";

type DownloadEventName =
  | "download-progress"
  | "download-completed"
  | "download-failed"
  | "install-progress"
  | "install-completed"
  | "install-failed";

const listeners = new Map<DownloadEventName, (event: { payload: unknown }) => void>();
const checkClientUpdate = vi.fn();
const getDefaultClient = vi.fn();
const listDownloadJobRecoveries = vi.fn();
const listInstallHistory = vi.fn();
const loadAppSettings = vi.fn();
const reportLocalSmokeResult = vi.fn();
const closeWindow = vi.fn();
const validateClientDir = vi.fn();
const upsertClientInstallation = vi.fn();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: closeWindow
  })
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: DownloadEventName, callback: (event: { payload: unknown }) => void) => {
    listeners.set(eventName, callback);
    return Promise.resolve(() => {
      listeners.delete(eventName);
    });
  }
}));

vi.mock("../../lib/tauri", async () => {
  const actual = await vi.importActual<typeof TauriApi>("../../lib/tauri");
  return {
    ...actual,
    checkClientUpdate: (...args: unknown[]) => checkClientUpdate(...args),
    getDefaultClient: (...args: unknown[]) => getDefaultClient(...args),
    installDownloadedUpdate: vi.fn(),
    isTauriRuntime: () => true,
    listDownloadJobRecoveries: (...args: unknown[]) => listDownloadJobRecoveries(...args),
    listInstallHistory: (...args: unknown[]) => listInstallHistory(...args),
    loadAppSettings: (...args: unknown[]) => loadAppSettings(...args),
    reportLocalSmokeResult: (...args: unknown[]) => reportLocalSmokeResult(...args),
    startUpdateDownload: vi.fn(),
    upsertClientInstallation: (...args: unknown[]) => upsertClientInstallation(...args),
    validateClientDir: (...args: unknown[]) => validateClientDir(...args)
  };
});

const defaultClient: ClientInstallation = {
  id: "client-current",
  client_id: "qmclient",
  display_name: "QmClient",
  install_dir: "D:/Games/QmClient",
  executable_path: "D:/Games/QmClient/DDNet.exe",
  storage_cfg_path: "D:/Games/QmClient/storage.cfg",
  data_dir: "D:/Games/QmClient/data",
  user_data_dir: null,
  version: "2.62.3",
  is_default: true,
  health: "ok",
  missing_items: [],
  install_source: "manual",
  confidence: "compatible",
  manager_owned: false,
  compatibility: {
    status: "supported",
    can_launch: true,
    launch_verified: true,
    last_launch_result: null,
    reasons: []
  },
  upstream_url: null,
  last_scanned_at: null
};

const externalJob: DownloadJob = {
  id: "download-external",
  client_installation_id: "client-other",
  client_id: "qmclient",
  channel: "stable",
  version: "9.9.9",
  asset_url: "https://example.com/QmClient.zip",
  sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  size: 100,
  status: "verified",
  downloaded_bytes: 100,
  cache_path: "D:/Cache/download-external.zip",
  error: null
};

describe("UpdatePanel event ownership", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    checkClientUpdate.mockResolvedValue(null);
    closeWindow.mockResolvedValue(undefined);
    reportLocalSmokeResult.mockResolvedValue(undefined);
    getDefaultClient.mockResolvedValue(defaultClient);
    loadAppSettings.mockResolvedValue({
      network_route: null,
      scan_excluded_paths: [],
      use_everything: false,
      close_panel_after_launch: true,
      auto_check_updates: false,
      advanced_manifest_url: null
    });
    listDownloadJobRecoveries.mockResolvedValue([]);
    listInstallHistory.mockResolvedValue([]);
    upsertClientInstallation.mockResolvedValue({
      ...defaultClient,
      id: "client-smoke-persisted",
      install_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
      executable_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/DDNet.exe",
      storage_cfg_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/storage.cfg",
      data_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/data",
      version: null,
      is_default: false
    });
    validateClientDir.mockResolvedValue({
      ...defaultClient,
      id: "client-smoke",
      install_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
      executable_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/DDNet.exe",
      storage_cfg_path: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/storage.cfg",
      data_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/data",
      version: null,
      is_default: false
    });
  });

  it("ignores download events for a different client installation", async () => {
    render(<UpdatePanel />);

    await waitFor(() => {
      expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-current");
    });

    await act(async () => {
      listeners.get("download-completed")?.({ payload: externalJob });
    });

    expect(screen.queryByText("9.9.9")).not.toBeInTheDocument();
    expect(screen.queryByText("下载完成，文件已通过校验，可以直接安装。")).not.toBeInTheDocument();
  });

  it("ignores install progress for jobs outside the current panel", async () => {
    render(<UpdatePanel />);

    await waitFor(() => {
      expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-current");
    });

    await act(async () => {
      listeners.get("install-progress")?.({ payload: "download-external" });
    });

    expect(screen.queryByText("正在安装更新，请保持客户端关闭。")).not.toBeInTheDocument();
  });

  it("uses the validated smoke client instead of the persisted default client in smoke mode", async () => {
    render(
      <UpdatePanel
        smokeAutomation={{
          clientInstallDir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
          manifestUrl: "http://127.0.0.1:18765/manifest.json",
          closeWindowOnFinish: false
        }}
      />
    );

    await waitFor(() => {
      expect(validateClientDir).toHaveBeenCalledWith(
        "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient"
      );
    });

    expect(screen.getByText("E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient")).toBeInTheDocument();
    expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-smoke-persisted");
    expect(listDownloadJobRecoveries).not.toHaveBeenCalledWith("client-current");
    expect(getDefaultClient).not.toHaveBeenCalled();
  });

  it("persists the smoke client as a non-default installation before loading artifacts", async () => {
    render(
      <UpdatePanel
        smokeAutomation={{
          clientInstallDir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
          manifestUrl: "http://127.0.0.1:18765/manifest.json",
          closeWindowOnFinish: false
        }}
      />
    );

    await waitFor(() => {
      expect(upsertClientInstallation).toHaveBeenCalledWith({
        install_dir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
        is_default: false
      });
    });
    expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-smoke-persisted");
    expect(listInstallHistory).toHaveBeenCalledWith("client-smoke-persisted");
  });

  it("closes the smoke window when result reporting fails after install completion", async () => {
    checkClientUpdate.mockImplementation(() => new Promise(() => undefined));
    reportLocalSmokeResult.mockRejectedValue(new Error("result path missing"));

    render(
      <UpdatePanel
        smokeAutomation={{
          clientInstallDir: "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient",
          manifestUrl: "http://127.0.0.1:18765/manifest.json",
          closeWindowOnFinish: true
        }}
      />
    );

    await waitFor(() => {
      expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-smoke-persisted");
    });

    await act(async () => {
      listeners.get("install-completed")?.({
        payload: {
          ...externalJob,
          id: "download-smoke",
          client_installation_id: "client-smoke-persisted",
          status: "completed"
        }
      });
    });

    await waitFor(() => {
      expect(reportLocalSmokeResult).toHaveBeenCalledWith({
        status: "succeeded",
        stage: "install",
        message: null
      });
    });
    await waitFor(() => {
      expect(closeWindow).toHaveBeenCalledTimes(1);
    });
  });
});
