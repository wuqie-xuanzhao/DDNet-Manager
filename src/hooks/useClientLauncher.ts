import { startTransition, useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { getErrorMessage } from "../lib/errors";
import {
  getLaunchReadiness,
  launchDefaultClient,
  reportLocalSmokeResult,
  upsertClientInstallation,
  validateClientDir
} from "../lib/tauri";
import type {
  AppSettings,
  ClientHealth,
  ClientInstallation,
  LauncherState,
  LaunchReadiness,
  LocalSmokeAutomationConfig
} from "../types";
import type { ClientTypeId } from "../components/games/GamesPanel";

function normalizeHealth(health: ClientHealth): string {
  switch (health) {
    case "ok":
      return "可启动";
    case "missing_executable":
      return "缺少 DDNet.exe";
    case "missing_storage_cfg":
      return "缺少 storage.cfg";
    case "missing_data_dir":
      return "缺少 data 目录";
  }
}

function clientTypeIdFromInstallation(client: ClientInstallation): ClientTypeId {
  if (
    (client.client_id === "ddnet" || client.client_id === "ddnet_vanilla") &&
    client.install_dir.toLowerCase().includes("steamapps")
  ) {
    return "ddnet-steam";
  }

  switch (client.client_id) {
    case "qmclient":
      return "qmclient";
    case "qmclient_nightly":
      return "qmclient-nightly";
    case "ddnet":
      return "ddnet";
    case "ddnet_vanilla":
      return "ddnet";
    case "taterclient":
      return "taterclient";
    case "bestclient":
      return "bestclient";
    case "cactusclient":
      return "cactusclient";
    default:
      return "third-party";
  }
}

const browserPreviewReadiness: LaunchReadiness = {
  client: null,
  can_launch: false,
  running: false,
  status_label: "浏览器预览",
  user_message: "当前为浏览器预览模式，启动能力需在 Tauri 桌面运行时验证。",
  blocking_reasons: ["缺少 Tauri IPC 注入"],
  checked_at: null
};

async function reportSmokeFailure(stage: string, message: string) {
  await reportLocalSmokeResult({
    status: "failed",
    stage,
    message
  });
}

async function closeSmokeWindowIfRequested(enabled: boolean) {
  if (enabled) {
    await getCurrentWindow().close();
  }
}

export function useClientLauncher(params: {
  appSettings: AppSettings;
  localSmokeAutomation: LocalSmokeAutomationConfig | null;
  onOpenUpdateView: () => void;
  tauriRuntime: boolean;
}) {
  const { appSettings, localSmokeAutomation, onOpenUpdateView, tauriRuntime } = params;
  const [runtimeLauncherState, setRuntimeLauncherState] = useState<LauncherState>("unconfigured");
  const [clientPath, setClientPath] = useState("");
  const [selectedClient, setSelectedClient] = useState<ClientInstallation | null>(null);
  const [selectedClientTypeId, setSelectedClientTypeId] = useState<ClientTypeId>("qmclient");
  const [runtimeErrorMessage, setRuntimeErrorMessage] = useState<string | null>(null);
  const [runtimeLaunchReadiness, setRuntimeLaunchReadiness] = useState<LaunchReadiness | null>(null);
  const validationRequestId = useRef(0);
  const localSmokeBootstrapStateRef = useRef<"idle" | "running" | "done">("idle");

  const markInvalid = (message: string, options?: { clearClient?: boolean }) => {
    if (options?.clearClient ?? true) {
      setSelectedClient(null);
    }
    setRuntimeLauncherState("error");
    setRuntimeErrorMessage(message);
  };

  const applyReadiness = useCallback((readiness: LaunchReadiness) => {
    setRuntimeLaunchReadiness(readiness);

    if (!readiness.client) {
      setSelectedClient(null);
      setRuntimeLauncherState("unconfigured");
      return readiness;
    }

    setSelectedClient(readiness.client);
    setClientPath(readiness.client.install_dir);
    setSelectedClientTypeId(clientTypeIdFromInstallation(readiness.client));
    if (readiness.running) {
      setRuntimeLauncherState("running");
    } else if (readiness.can_launch) {
      setRuntimeLauncherState("ready");
    } else {
      setRuntimeLauncherState("error");
    }
    setRuntimeErrorMessage(readiness.can_launch || readiness.running ? null : readiness.user_message);
    return readiness;
  }, []);

  const refreshLaunchReadiness = useCallback(async () => applyReadiness(await getLaunchReadiness()), [applyReadiness]);

  useEffect(() => {
    if (!tauriRuntime) {
      return;
    }

    let alive = true;

    void getLaunchReadiness()
      .then((readiness) => {
        if (alive) {
          applyReadiness(readiness);
        }
      })
      .catch((error) => {
        if (alive) {
          setRuntimeErrorMessage(getErrorMessage(error));
        }
      });

    return () => {
      alive = false;
    };
  }, [applyReadiness, tauriRuntime]);

  const validateClientPath = useCallback(async (path: string, options?: { persistDefault?: boolean }) => {
    if (!tauriRuntime) {
      markInvalid("当前为浏览器预览模式，目录验证需在 Tauri 桌面运行时验证。", {
        clearClient: false
      });
      return false;
    }

    const normalizedPath = path.trim();
    if (!normalizedPath) {
      markInvalid("请先选择路径。");
      return false;
    }

    setClientPath(normalizedPath);
    const requestId = validationRequestId.current + 1;
    validationRequestId.current = requestId;

    setRuntimeLauncherState("validating");
    setRuntimeErrorMessage(null);

    try {
      const installation = await validateClientDir(normalizedPath);
      if (requestId !== validationRequestId.current) {
        return false;
      }

      if (installation.health !== "ok") {
        setSelectedClient(installation);
        setClientPath(installation.install_dir);
        markInvalid(`不可启动：${normalizeHealth(installation.health)}。`, {
          clearClient: false
        });
        return false;
      }

      const shouldPersistDefault = options?.persistDefault ?? true;
      const savedInstallation = await upsertClientInstallation({
        install_dir: installation.install_dir,
        is_default: shouldPersistDefault
      });

      setSelectedClient(savedInstallation);
      setClientPath(savedInstallation.install_dir);
      setRuntimeLauncherState("ready");
      if (shouldPersistDefault) {
        await refreshLaunchReadiness();
      }
      return true;
    } catch (error) {
      if (requestId !== validationRequestId.current) {
        return false;
      }

      markInvalid(`验证失败：${getErrorMessage(error)}`);
      return false;
    }
  }, [refreshLaunchReadiness, tauriRuntime]);

  useEffect(() => {
    if (!tauriRuntime || !localSmokeAutomation) {
      return;
    }
    if (localSmokeBootstrapStateRef.current === "running" || localSmokeBootstrapStateRef.current === "done") {
      return;
    }

    localSmokeBootstrapStateRef.current = "running";

    void (async () => {
      setClientPath(localSmokeAutomation.clientInstallDir);
      const validated = await validateClientPath(localSmokeAutomation.clientInstallDir, {
        persistDefault: false
      });
      if (!validated) {
        localSmokeBootstrapStateRef.current = "done";
        try {
          await reportSmokeFailure("bootstrap", "验证失败：本地 smoke 启动客户端目录不可用。");
          await closeSmokeWindowIfRequested(localSmokeAutomation.closeWindowOnFinish);
        } catch (error) {
          setRuntimeErrorMessage(`本地 smoke 结果写入失败：${getErrorMessage(error)}`);
          try {
            await closeSmokeWindowIfRequested(localSmokeAutomation.closeWindowOnFinish);
          } catch (closeError) {
            setRuntimeErrorMessage(`本地 smoke 结果写入失败，且窗口关闭失败：${getErrorMessage(closeError)}`);
          }
        }
        return;
      }
      localSmokeBootstrapStateRef.current = "done";
      startTransition(onOpenUpdateView);
    })();
  }, [localSmokeAutomation, onOpenUpdateView, tauriRuntime, validateClientPath]);

  const handleClientPathChange = (value: string) => {
    validationRequestId.current += 1;
    setClientPath(value);
    setRuntimeErrorMessage(null);

    if (selectedClient && value !== selectedClient.install_dir) {
      setSelectedClient(null);
      setRuntimeLauncherState("unconfigured");
    } else if (!value.trim()) {
      setSelectedClient(null);
      setRuntimeLauncherState("unconfigured");
    }
  };

  const handleBrowse = async () => {
    if (!tauriRuntime) {
      markInvalid("当前为浏览器预览模式，目录选择需在 Tauri 桌面运行时验证。", {
        clearClient: false
      });
      return;
    }

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择客户端目录"
      });

      if (typeof selected !== "string") {
        return;
      }

      setClientPath(selected);
      setSelectedClient(null);
      setRuntimeErrorMessage(null);
      await validateClientPath(selected);
    } catch (error) {
      markInvalid(`选择失败：${getErrorMessage(error)}`);
    }
  };

  const handleValidate = async () => {
    await validateClientPath(clientPath);
  };

  const handlePrimaryAction = async () => {
    if (!tauriRuntime) {
      markInvalid("当前为浏览器预览模式，启动能力需在 Tauri 桌面运行时验证。", {
        clearClient: false
      });
      return;
    }

    if (runtimeLauncherState === "validating" || runtimeLauncherState === "launching") {
      return;
    }

    if (!selectedClient || selectedClient.health !== "ok") {
      if (!clientPath.trim()) {
        await handleBrowse();
        return;
      }

      await handleValidate();
      return;
    }

    setRuntimeLauncherState("launching");
    setRuntimeErrorMessage(null);

    try {
      await launchDefaultClient();
      let launchWarning: string | null = null;
      if (appSettings.close_panel_after_launch && tauriRuntime) {
        try {
          await getCurrentWindow().minimize();
        } catch (error) {
          launchWarning = `启动成功，但最小化启动器失败：${getErrorMessage(error)}`;
        }
      }
      try {
        await refreshLaunchReadiness();
      } catch (error) {
        launchWarning = `启动成功，但运行状态刷新失败：${getErrorMessage(error)}`;
      }
      setRuntimeLauncherState("running");
      setRuntimeErrorMessage(launchWarning);
    } catch (error) {
      markInvalid(`启动失败：${getErrorMessage(error)}`);
    }
  };

  const handleBackgroundValidationError = (message: string) => {
    markInvalid(message, { clearClient: false });
  };

  const selectClientType = (id: ClientTypeId) => {
    if (id === selectedClientTypeId) {
      return;
    }

    startTransition(() => {
      setSelectedClientTypeId(id);
      setRuntimeErrorMessage(null);
      setClientPath("");
      setSelectedClient(null);
      setRuntimeLauncherState("unconfigured");
    });
  };

  const launchReadiness = tauriRuntime ? runtimeLaunchReadiness : browserPreviewReadiness;
  const launcherState = tauriRuntime ? runtimeLauncherState : "unconfigured";
  const errorMessage = tauriRuntime ? runtimeErrorMessage : null;
  const visibleSelectedClient = tauriRuntime ? selectedClient : null;

  return {
    clientPath,
    errorMessage,
    handleBackgroundValidationError,
    handleBrowse,
    handleClientPathChange,
    handlePrimaryAction,
    handleValidate,
    launchReadiness,
    launcherState,
    reportSmokeFailure,
    selectedClient: visibleSelectedClient,
    selectedClientTypeId,
    selectClientType
  };
}
