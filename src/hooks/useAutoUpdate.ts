import { useEffect, useMemo, useRef, useState } from "react";
import { getErrorMessage } from "../lib/errors";
import { deriveAutoUpdateView, type AutoUpdateMode, type AutoUpdateSnapshot, type AutoUpdateViewState } from "../lib/updateLogic";
import { checkClientUpdate } from "../lib/tauri";
import type { AppSettings, ClientInstallation } from "../types";
import type { SettingsSaveState } from "./useAppSettings";

export type AutoUpdateState = AutoUpdateViewState;

export function useAutoUpdate(params: {
  tauriRuntime: boolean;
  selectedClient: ClientInstallation | null;
  savedAppSettings: AppSettings;
  settingsState: SettingsSaveState;
}) {
  const { savedAppSettings, selectedClient, settingsState, tauriRuntime } = params;
  const [autoUpdateSnapshot, setAutoUpdateSnapshot] = useState<AutoUpdateSnapshot | null>(null);
  const autoUpdateRequestKey = useRef<string | null>(null);

  const requestKey = useMemo(() => {
    if (!selectedClient || selectedClient.health !== "ok") {
      return null;
    }

    return `${selectedClient.id}:${selectedClient.version ?? ""}:${savedAppSettings.advanced_manifest_url ?? ""}:${JSON.stringify(
      savedAppSettings.network_route
    )}`;
  }, [savedAppSettings.advanced_manifest_url, savedAppSettings.network_route, selectedClient]);

  const mode: AutoUpdateMode = !savedAppSettings.auto_check_updates
    ? "disabled"
    : !tauriRuntime || !selectedClient || selectedClient.health !== "ok"
      ? "idle"
      : settingsState === "loading"
        ? "loading"
        : "ready";

  useEffect(() => {
    if (mode !== "ready") {
      autoUpdateRequestKey.current = null;
    }
  }, [mode]);

  useEffect(() => {
    if (mode !== "ready" || !selectedClient || !requestKey) {
      return;
    }

    if (autoUpdateRequestKey.current === requestKey) {
      return;
    }

    autoUpdateRequestKey.current = requestKey;
    let alive = true;

    void checkClientUpdate({
      client_id: selectedClient.client_id,
      channel: "stable",
      manifest_url: savedAppSettings.advanced_manifest_url,
      network_route: savedAppSettings.network_route,
      use_manifest_source: Boolean(savedAppSettings.advanced_manifest_url)
    })
      .then((result) => {
        if (!alive) {
          return;
        }

        setAutoUpdateSnapshot({ requestKey, update: result, error: null });
      })
      .catch((error) => {
        if (!alive) {
          return;
        }
        setAutoUpdateSnapshot({ requestKey, update: null, error: getErrorMessage(error) });
      });

    return () => {
      alive = false;
    };
  }, [mode, requestKey, savedAppSettings.advanced_manifest_url, savedAppSettings.network_route, selectedClient]);

  const autoUpdateView = deriveAutoUpdateView({
    mode,
    requestKey,
    snapshot: autoUpdateSnapshot
  });

  return {
    autoUpdate: autoUpdateView.autoUpdate,
    autoUpdateError: autoUpdateView.autoUpdateError,
    autoUpdateState: autoUpdateView.autoUpdateState
  };
}
