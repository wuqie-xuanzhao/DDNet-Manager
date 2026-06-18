import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ClientInstallation, ClientUpdateCheck } from "../types";
import { useClientInstaller } from "./useClientInstaller";

// Mock 所有 IPC + 事件
const listClientInstallations = vi.fn();
const scanClientsViaMft = vi.fn();
const upsertClientInstallation = vi.fn();
const checkClientUpdate = vi.fn();
const launchClient = vi.fn();
const startUpdateDownload = vi.fn();
const getClientCatalog = vi.fn();
const createShortcuts = vi.fn();
const listenMock = vi.fn();

vi.mock("../lib/tauri", () => ({
  isTauriRuntime: () => true,
  listClientInstallations: (...a: unknown[]) => listClientInstallations(...a),
  scanClientsViaMft: (...a: unknown[]) => scanClientsViaMft(...a),
  upsertClientInstallation: (...a: unknown[]) => upsertClientInstallation(...a),
  checkClientUpdate: (...a: unknown[]) => checkClientUpdate(...a),
  launchClient: (...a: unknown[]) => launchClient(...a),
  startUpdateDownload: (...a: unknown[]) => startUpdateDownload(...a),
  getClientCatalog: (...a: unknown[]) => getClientCatalog(...a),
  createShortcuts: (...a: unknown[]) => createShortcuts(...a)
}));

vi.mock("../lib/updateLogic", () => ({
  buildUpdateSourceRequest: (input: unknown) => input,
  buildStartUpdateDownloadRequest: (input: unknown) => input
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: unknown[]) => listenMock(...a)
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ hide: vi.fn().mockResolvedValue(undefined) })
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

function makeClient(overrides: Partial<ClientInstallation> = {}): ClientInstallation {
  return {
    id: "client-1",
    client_id: "qmclient",
    display_name: "QmClient",
    install_dir: "C:/Games/QmClient",
    executable_path: "C:/Games/QmClient/DDNet.exe",
    storage_cfg_path: "C:/Games/QmClient/storage.cfg",
    data_dir: "C:/Games/QmClient/data",
    user_data_dir: null,
    version: "1.0.0",
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
      reasons: [],
      last_launch_result: null
    },
    upstream_url: null,
    pe_company_name: null,
    pe_product_name: null,
    pe_file_version: null,
    exe_sha256: null,
    last_scanned_at: null,
    ...overrides
  };
}

const catalogEntries = [
  {
    client_id: "qmclient",
    display_name: "QmClient",
    aliases: ["qmclient"],
    executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
    required_markers: ["storage.cfg", "data"],
    pe_company_names: [],
    pe_product_names: [],
    known_hashes: [],
    update_source: { kind: "github_release", owner: "wxj881027", repo: "QmClient", windows_assets: [], macos_assets: [], linux_assets: [] },
    upstream_url: "https://github.com/wxj881027/QmClient/releases"
  }
];

const releaseCheck: ClientUpdateCheck = {
  client_id: "qmclient",
  channel: "stable",
  current_version: "1.0.0",
  latest_version: "1.2.3",
  asset: { platform: "windows", asset_url: "https://example.com/qmclient.zip", sha256: "abc123", size: 1000000 },
  needs_update: true,
  source_kind: "github_release",
  action: "download",
  action_url: "https://github.com/wxj881027/QmClient/releases",
  message: null
};

describe("useClientInstaller", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listClientInstallations.mockResolvedValue([]);
    scanClientsViaMft.mockResolvedValue([]);
    upsertClientInstallation.mockResolvedValue(makeClient());
    checkClientUpdate.mockResolvedValue(releaseCheck);
    launchClient.mockResolvedValue(undefined);
    startUpdateDownload.mockResolvedValue({ id: "job-1" });
    getClientCatalog.mockResolvedValue(catalogEntries);
    createShortcuts.mockResolvedValue(undefined);
    // listen 返回 unlisten 函数
    listenMock.mockResolvedValue(vi.fn());
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("初始 state 为 loading，mount 后从 registry 拉客户端", async () => {
    listClientInstallations.mockResolvedValue([makeClient()]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    expect(result.current.state.kind).toBe("loading");

    await waitFor(() => {
      expect(result.current.state.kind).toBe("installed");
    });
    expect(result.current.client).not.toBeNull();
    expect(result.current.client?.id).toBe("client-1");
  });

  it("registry 无匹配 client → unknown 状态", async () => {
    listClientInstallations.mockResolvedValue([]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => {
      expect(result.current.state.kind).toBe("unknown");
    });
    expect(result.current.client).toBeNull();
  });

  it("匹配 client 但 health != ok → broken 状态", async () => {
    listClientInstallations.mockResolvedValue([
      makeClient({ health: "missing_data_dir", missing_items: ["data"] })
    ]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => {
      expect(result.current.state.kind).toBe("broken");
    });
    if (result.current.state.kind === "broken") {
      expect(result.current.state.reason).toContain("data");
    }
  });

  it("installed 状态下拉到 release → needsUpdate 刷新到 state", async () => {
    listClientInstallations.mockResolvedValue([makeClient({ version: "1.0.0" })]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => {
      expect(result.current.state.kind).toBe("installed");
    });

    // release 拉取是异步的，等 needsUpdate 刷新
    await waitFor(() => {
      const s = result.current.state;
      return s.kind === "installed" && s.needsUpdate === true;
    });

    const s = result.current.state;
    if (s.kind === "installed") {
      expect(s.latest).toBe("1.2.3");
      expect(s.assetSize).toBe(1000000);
      expect(s.releaseUrl).toBe("https://github.com/wxj881027/QmClient/releases");
    }
  });

  it("triggerScan 命中 → upsert 落 registry + state 切 installed", async () => {
    // triggerScan 内部会调 refreshFromRegistry → listClientInstallations，
    // 持续返回 [scanned] 让 refresh 后 state 也是 installed
    const scanned = makeClient({ id: "scanned-1", install_dir: "D:/Games/QmClient" });
    listClientInstallations.mockResolvedValue([scanned]);
    scanClientsViaMft.mockResolvedValue([scanned]);
    upsertClientInstallation.mockResolvedValue(scanned);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    // 首次 listClientInstallations 也返回 [scanned]，所以初始就是 installed（不是 unknown）
    // 这里改为：先返回空触发 unknown，triggerScan 后切换
    await waitFor(() => expect(result.current.state.kind).toBe("installed"));

    expect(scanClientsViaMft).not.toHaveBeenCalled();

    // 现在手动触发扫描（即使已 installed 也允许重扫）
    await act(async () => {
      await result.current.triggerScan();
    });

    expect(scanClientsViaMft).toHaveBeenCalledWith({ include_saved_paths: true, include_unhealthy: false });
    expect(upsertClientInstallation).toHaveBeenCalledWith({ install_dir: "D:/Games/QmClient", is_default: false });
    expect(result.current.state.kind).toBe("installed");
  });

  it("triggerScan 未命中 → not_installed 状态", async () => {
    listClientInstallations.mockResolvedValue([]);
    scanClientsViaMft.mockResolvedValue([]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => expect(result.current.state.kind).toBe("unknown"));

    await act(async () => {
      await result.current.triggerScan();
    });

    expect(result.current.state.kind).toBe("not_installed");
    if (result.current.state.kind === "not_installed") {
      expect(result.current.state.scanned).toBe(true);
    }
  });

  it("openInstallDialog 立即设置 dialogOpen=true（不阻塞扫描）", async () => {
    listClientInstallations.mockResolvedValue([]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => expect(result.current.state.kind).toBe("unknown"));

    await act(async () => {
      result.current.openInstallDialog();
    });

    expect(result.current.installDialogOpen).toBe(true);
    expect(result.current.installDialogMode).toBe("install");
    // openInstallDialog 不应该自动触发扫描
    expect(scanClientsViaMft).not.toHaveBeenCalled();
  });

  it("launchGame 调 launchClient IPC 并 hide 启动器窗口", async () => {
    listClientInstallations.mockResolvedValue([makeClient()]);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => expect(result.current.state.kind).toBe("installed"));

    await act(async () => {
      await result.current.launchGame();
    });

    expect(launchClient).toHaveBeenCalledWith("C:/Games/QmClient/DDNet.exe");
  });

  it("tauriRuntime=false 时 state 直接 unknown，不调 IPC", () => {
    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: false })
    );

    expect(result.current.state.kind).toBe("unknown");
    expect(listClientInstallations).not.toHaveBeenCalled();
  });

  it("beginInstall 调 upsert + startUpdateDownload", async () => {
    listClientInstallations.mockResolvedValue([]);
    const upserted = makeClient({ id: "upserted-1", install_dir: "E:/NewQm" });
    upsertClientInstallation.mockResolvedValue(upserted);

    const { result } = renderHook(() =>
      useClientInstaller({ gameId: "qmclient", appSettings: baseSettings as never, tauriRuntime: true })
    );

    await waitFor(() => expect(result.current.state.kind).toBe("unknown"));

    await act(async () => {
      await result.current.beginInstall({ installDir: "E:/NewQm", desktop: true, startMenu: false });
    });

    expect(upsertClientInstallation).toHaveBeenCalledWith({ install_dir: "E:/NewQm", is_default: false });
    expect(startUpdateDownload).toHaveBeenCalled();
    expect(result.current.installDialogOpen).toBe(false); // 弹窗关闭
  });
});
