import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
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
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const requestKey = useMemo(() => {
    if (!selectedClient || selectedClient.health !== "ok") {
      return null;
    }

    return `${selectedClient.id}:${selectedClient.version ?? ""}:${JSON.stringify(
      savedAppSettings.network_route
    )}`;
  }, [savedAppSettings.network_route, selectedClient]);

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

    let alive = true;

    const performCheck = () => {
      if (!alive) return;
      void checkClientUpdate({
        client_id: selectedClient.client_id,
        channel: "stable",
        manifest_url: null,
        network_route: savedAppSettings.network_route,
        use_manifest_source: false
      })
        .then((result) => {
          if (!alive) return;
          setAutoUpdateSnapshot({ requestKey, update: result, error: null });
          // 后台定时检查发现新版本时弹出 toast 通知
          if (result.reason === "none" && result.action === "download") {
            toast.info(`${selectedClient.display_name} 有新版本可用`, {
              description: `最新版本: ${result.latest_version ?? "未知"}`,
              duration: 5000,
            });
          }
        })
        .catch((error) => {
          if (!alive) return;
          setAutoUpdateSnapshot({ requestKey, update: null, error: getErrorMessage(error) });
        });
    };

    const startInterval = () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
      intervalRef.current = setInterval(() => {
        performCheck();
      }, 60 * 60 * 1000);
    };

    const stopInterval = () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };

    // 首次检查（requestKey 变化时）
    if (autoUpdateRequestKey.current !== requestKey) {
      autoUpdateRequestKey.current = requestKey;
      performCheck();
    }

    // 定时后台检查：每小时一次
    startInterval();

    const handleVisibilityChange = () => {
      if (!alive) return;
      if (document.hidden) {
        stopInterval();
      } else {
        // 恢复可见时补检一次，然后重启 interval
        performCheck();
        startInterval();
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      alive = false;
      stopInterval();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [mode, requestKey, savedAppSettings.network_route, selectedClient]);

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
