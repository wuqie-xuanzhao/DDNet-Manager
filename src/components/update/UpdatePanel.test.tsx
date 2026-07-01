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
const rollbackClientInstallation = vi.fn();
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
    rollbackClientInstallation: (...args: unknown[]) => rollbackClientInstallation(...args),
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
  pe_company_name: null,
  pe_product_name: null,
  pe_file_version: null,
  exe_sha256: null,
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

const mockSettings = {
  network_route: null,
  scan_excluded_paths: [],
  scan_max_results: null,
  scan_timeout_secs: null,
  close_panel_after_launch: true,
  auto_check_updates: false,
  autostart: false,
  exit_game_show_launcher: true,
  close_behavior: "minimize_to_tray",
  allow_silent_update: true,
  extra_trusted_hosts: [],
  mirror_prefixes: [],
  has_scanned_clients: true
};
const mockOnUpdateSettings = vi.fn().mockResolvedValue(undefined);

describe("UpdatePanel event ownership", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    // checkClientUpdate 不再返回 null：默认返回"已是最新版"，避免触发 error 分支。
    checkClientUpdate.mockResolvedValue({
      client_id: "qmclient",
      channel: "stable",
      current_version: "1.0.0",
      latest_version: "1.0.0",
      asset: { platform: "windows-x86_64", asset_url: "", sha256: "", size: 0 },
      needs_update: false,
      source_kind: "github_release",
      action: "none",
      action_url: null,
      message: null,
      reason: "none"
    });
    closeWindow.mockResolvedValue(undefined);
    reportLocalSmokeResult.mockResolvedValue(undefined);
    rollbackClientInstallation.mockReset();
    getDefaultClient.mockResolvedValue(defaultClient);
    loadAppSettings.mockResolvedValue(mockSettings);
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
    render(<UpdatePanel settings={mockSettings} onUpdateSettings={mockOnUpdateSettings} />);

    await waitFor(() => {
      expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-current");
    });

    await act(async () => {
      listeners.get("download-completed")?.({ payload: externalJob });
    });

    expect(screen.queryByText("9.9.9")).not.toBeInTheDocument();
    expect(screen.queryByText("已校验")).not.toBeInTheDocument();
  });

  it("ignores install progress for jobs outside the current panel", async () => {
    render(<UpdatePanel settings={mockSettings} onUpdateSettings={mockOnUpdateSettings} />);

    await waitFor(() => {
      expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-current");
    });

    await act(async () => {
      listeners.get("install-progress")?.({ payload: "download-external" });
    });

    expect(screen.queryByText("安装中")).not.toBeInTheDocument();
  });

  it("uses the validated smoke client instead of the persisted default client in smoke mode", async () => {
    render(
      <UpdatePanel
        settings={mockSettings}
        onUpdateSettings={mockOnUpdateSettings}
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
      expect(screen.getByText("E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient")).toBeInTheDocument();
    });

    expect(listDownloadJobRecoveries).toHaveBeenCalledWith("client-smoke-persisted");
    expect(listDownloadJobRecoveries).not.toHaveBeenCalledWith("client-current");
    expect(getDefaultClient).not.toHaveBeenCalled();
  });

  it("persists the smoke client as a non-default installation before loading artifacts", async () => {
    render(
      <UpdatePanel
        settings={mockSettings}
        onUpdateSettings={mockOnUpdateSettings}
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
        settings={mockSettings}
        onUpdateSettings={mockOnUpdateSettings}
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

  it("exposes rollback entry for completed install history with rollback_path", async () => {
    // 守卫缺口3：成功安装后保留的 rollback 目录必须能通过 IPC 回滚，
    // 且只在 status=completed + rollback_path 非空时显示按钮。
    const completedWithRollback = {
      id: "install-completed-1",
      job_id: "download-1",
      client_installation_id: "client-current",
      client_id: "qmclient",
      version: "2.62.4",
      asset_url: "https://example.com/QmClient.zip",
      package_kind: "zip",
      status: "completed" as const,
      rollback_path: "D:/Games/QmClient.ddnet-manager-rollback-install-completed-1",
      error: null,
      completed_at: "2026-07-01T00:00:00Z"
    };
    const rolledBackRecord = {
      id: "install-rolled-1",
      job_id: "download-2",
      client_installation_id: "client-current",
      client_id: "qmclient",
      version: "2.62.5",
      asset_url: "https://example.com/QmClient.zip",
      package_kind: "zip",
      status: "rolled_back" as const,
      rollback_path: "D:/Games/QmClient.ddnet-manager-rollback-install-rolled-1",
      error: null,
      completed_at: "2026-07-01T01:00:00Z"
    };
    const failedRecord = {
      id: "install-failed-1",
      job_id: "download-3",
      client_installation_id: "client-current",
      client_id: "qmclient",
      version: "2.62.6",
      asset_url: "https://example.com/QmClient.zip",
      package_kind: "zip",
      status: "failed" as const,
      rollback_path: null,
      error: "checksum mismatch",
      completed_at: "2026-07-01T02:00:00Z"
    };
    listInstallHistory.mockResolvedValue([completedWithRollback, rolledBackRecord, failedRecord]);

    render(<UpdatePanel settings={mockSettings} onUpdateSettings={mockOnUpdateSettings} />);

    // 展开历史记录卡片
    await waitFor(() => {
      expect(listInstallHistory).toHaveBeenCalledWith("client-current");
    });
    const toggle = await screen.findByText(/展开（3）/);
    await act(async () => {
      toggle.click();
    });

    // 只有 completed + rollback_path 非空的记录应显示回滚按钮
    const rollbackButtons = screen.getAllByRole("button", { name: "回滚到此版本之前" });
    expect(rollbackButtons).toHaveLength(1);

    // 点击回滚应调用 IPC，并把客户端记录刷成回滚后版本（version=null）
    rollbackClientInstallation.mockResolvedValue({ ...defaultClient, version: null });
    await act(async () => {
      rollbackButtons[0].click();
    });
    await waitFor(() => {
      expect(rollbackClientInstallation).toHaveBeenCalledWith("install-completed-1");
    });
    await waitFor(() => {
      // notice 在客户端卡片和 visibleNotice 两处都渲染，用 getAllByText 兜底。
      expect(screen.getAllByText("已回滚到上一版本").length).toBeGreaterThan(0);
    });
  });
});
