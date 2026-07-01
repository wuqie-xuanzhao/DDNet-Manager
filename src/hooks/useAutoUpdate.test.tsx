import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../lib/settings";
import type { ClientInstallation } from "../types";
import { useAutoUpdate } from "./useAutoUpdate";

const checkClientUpdate = vi.fn();

vi.mock("../lib/tauri", () => ({
  checkClientUpdate: (...args: unknown[]) => checkClientUpdate(...args)
}));

const selectedClient: ClientInstallation = {
  id: "qmclient-main",
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

describe("useAutoUpdate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("retries the same request key after auto check is disabled and enabled again", async () => {
    checkClientUpdate.mockRejectedValueOnce(new Error("network down"));
    // checkClientUpdate 不再返回 null：用 reason="none" + needs_update=false 表示"已是最新版"。
    checkClientUpdate.mockResolvedValueOnce({
      client_id: "qmclient",
      channel: "stable",
      current_version: "2.62.3",
      latest_version: "2.62.3",
      asset: { platform: "windows-x86_64", asset_url: "", sha256: "", size: 0 },
      needs_update: false,
      source_kind: "github_release",
      action: "none",
      action_url: null,
      message: null,
      reason: "none"
    });

    const { rerender, result } = renderHook(
      (props: { autoCheck: boolean }) =>
        useAutoUpdate({
          tauriRuntime: true,
          selectedClient,
          savedAppSettings: {
            ...defaultAppSettings,
            auto_check_updates: props.autoCheck
          },
          settingsState: "idle"
        }),
      {
        initialProps: {
          autoCheck: true
        }
      }
    );

    await waitFor(() => {
      expect(result.current.autoUpdateState).toBe("error");
    });
    expect(checkClientUpdate).toHaveBeenCalledTimes(1);

    rerender({ autoCheck: false });
    await waitFor(() => {
      expect(result.current.autoUpdateState).toBe("disabled");
    });

    rerender({ autoCheck: true });
    await waitFor(() => {
      expect(result.current.autoUpdateState).toBe("current");
    });
    expect(checkClientUpdate).toHaveBeenCalledTimes(2);
  });
});
