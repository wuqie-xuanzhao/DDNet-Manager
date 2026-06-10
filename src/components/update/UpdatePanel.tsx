import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  checkClientUpdate,
  getDefaultClient,
  installDownloadedUpdate,
  isTauriRuntime,
  listDownloadJobRecoveries,
  listInstallHistory,
  reportLocalSmokeResult,
  startUpdateDownload,
  upsertClientInstallation,
  validateClientDir
} from "../../lib/tauri";
import type {
  ClientInstallation,
  ClientUpdateCheck,
  DownloadJob,
  DownloadJobRecovery,
  InstallHistoryRecord,
  LocalSmokeAutomationConfig,
  AppSettings
} from "../../types";
import { getUpdateErrorMessage } from "../../lib/errors";
import {
  buildStartUpdateDownloadRequest,
  buildUpdateSourceRequest,
  networkRouteLabel,
  progressPercent,
  resolveUpdateManifestInput
} from "../../lib/updateLogic";
import { updateNetworkRoute } from "../../lib/settings";

type SmokeStage = "bootstrap" | "check" | "download" | "install";

type SmokePhase =
  | "idle"
  | "checking"
  | "waiting_download"
  | "downloading"
  | "waiting_install"
  | "installing"
  | "succeeded"
  | "failed";

function formatAssetSize(size: number) {
  if (size >= 1024 * 1024) {
    return `${(size / (1024 * 1024)).toFixed(2)} MB`;
  }

  if (size >= 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${size} B`;
}

function formatCompletedAt(value: string | null) {
  if (!value) {
    return "未记录";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString("zh-CN", { hour12: false });
}

function downloadStatusLabel(status: DownloadJob["status"]) {
  switch (status) {
    case "pending":
      return "等待下载";
    case "downloading":
      return "下载中";
    case "verified":
      return "文件已校验";
    case "installing":
      return "安装中";
    case "completed":
      return "已安装";
    case "canceled":
      return "已取消";
    case "failed":
      return "失败";
  }
}

function recoveryStateLabel(state: DownloadJobRecovery["cache_state"]) {
  switch (state) {
    case "missing":
      return "缓存缺失";
    case "present":
      return "缓存存在";
    case "verified":
      return "缓存已校验";
    case "corrupted":
      return "缓存损坏";
  }
}

function installHistoryStatusLabel(status: InstallHistoryRecord["status"]) {
  switch (status) {
    case "completed":
      return "安装完成";
    case "failed":
      return "安装失败";
  }
}

function updateSourceLabel(source: ClientUpdateCheck["source_kind"]) {
  switch (source) {
    case "github_release":
      return "GitHub Release";
    case "website":
      return "官网";
    case "manifest":
      return "Manifest";
    case "ddnet_official":
      return "DDNet 官网";
    case "none":
      return "无自动来源";
  }
}

export function UpdatePanel(props: {
  smokeAutomation?: LocalSmokeAutomationConfig | null;
  settings: AppSettings;
  onUpdateSettings: (settings: AppSettings) => Promise<void>;
}) {
  const tauriRuntime = isTauriRuntime();
  const smokeAutomation = props.smokeAutomation ?? null;
  const smokeEnabled = smokeAutomation !== null;
  const smokeClientInstallDir = smokeAutomation?.clientInstallDir.trim() ?? "";
  const smokeManifestUrl = smokeAutomation?.manifestUrl.trim() ?? "";
  const smokeCloseWindowOnFinish = smokeAutomation?.closeWindowOnFinish ?? false;
  const [hydratedKey, setHydratedKey] = useState<string | null>(null);
  const [channel, setChannel] = useState("stable");
  const [client, setClient] = useState<ClientInstallation | null>(null);
  const [update, setUpdate] = useState<ClientUpdateCheck | null>(null);
  const [job, setJob] = useState<DownloadJob | null>(null);
  const [recoveries, setRecoveries] = useState<DownloadJobRecovery[]>([]);
  const [installHistory, setInstallHistory] = useState<InstallHistoryRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const latestRequestIdRef = useRef(0);
  const currentClientIdRef = useRef<string | null>(null);
  const smokePhaseRef = useRef<SmokePhase>("idle");
  const smokeReportedRef = useRef(false);
  const smokeFailureStageRef = useRef<SmokeStage>("bootstrap");
  const hydrationKey = `${tauriRuntime ? "tauri" : "browser"}:${smokeEnabled ? smokeManifestUrl : "manual"}`;

  const activeRouteMode = props.settings.network_route?.mode ?? "direct";
  const activeRouteUrl = props.settings.network_route?.proxy_prefix_url ?? props.settings.network_route?.mirror_template ?? "";

  const manifestInput = resolveUpdateManifestInput({
    smokeEnabled,
    smokeManifestUrl,
    useManifestSource: props.settings.advanced_manifest_url !== null,
    manifestUrl: props.settings.advanced_manifest_url ?? ""
  });
  const activeUseManifestSource = manifestInput.useManifestSource;
  const activeManifestUrl = manifestInput.manifestUrl;

  const loadClientArtifacts = useCallback(async (clientInstallationId: string) => {
    const [nextRecoveries, nextHistory] = await Promise.all([
      listDownloadJobRecoveries(clientInstallationId),
      listInstallHistory(clientInstallationId)
    ]);
    setRecoveries(nextRecoveries);
    setInstallHistory(nextHistory);
  }, [setInstallHistory, setRecoveries]);

  const refreshClientArtifacts = useCallback(async () => {
    const clientInstallationId = currentClientIdRef.current;
    if (!clientInstallationId) {
      setRecoveries([]);
      setInstallHistory([]);
      return;
    }

    await loadClientArtifacts(clientInstallationId);
  }, [loadClientArtifacts, setInstallHistory, setRecoveries]);

  const completeSmoke = useCallback(
    async (status: "succeeded" | "failed", stage: SmokeStage, message?: string | null) => {
      if (!smokeEnabled || smokeReportedRef.current) {
        return;
      }

      smokeReportedRef.current = true;
      smokePhaseRef.current = status === "succeeded" ? "succeeded" : "failed";

      try {
        await reportLocalSmokeResult({
          status,
          stage,
          message
        });
      } catch (err) {
        smokeReportedRef.current = true;
        setError(getUpdateErrorMessage(err));
        if (smokeCloseWindowOnFinish) {
          try {
            await getCurrentWindow().close();
          } catch (closeError) {
            setError(getUpdateErrorMessage(closeError));
          }
        }
        return;
      }

      if (!smokeCloseWindowOnFinish) {
        return;
      }

      try {
        await getCurrentWindow().close();
      } catch (err) {
        setError(getUpdateErrorMessage(err));
      }
    },
    [setError, smokeCloseWindowOnFinish, smokeEnabled]
  );

  useEffect(() => {
    if (!tauriRuntime) {
      return;
    }

    let alive = true;

    const hydrate = async () => {
      try {
        const nextClient = smokeEnabled
          ? await upsertClientInstallation({
              install_dir: (await validateClientDir(smokeClientInstallDir)).install_dir,
              is_default: false
            })
          : await getDefaultClient();
        if (!alive) {
          return;
        }

        setClient(nextClient);
        currentClientIdRef.current = nextClient?.id ?? null;
        if (nextClient) {
          const [nextRecoveries, nextHistory] = await Promise.all([
            listDownloadJobRecoveries(nextClient.id),
            listInstallHistory(nextClient.id)
          ]);
          if (!alive) {
            return;
          }
          setRecoveries(nextRecoveries);
          setInstallHistory(nextHistory);
        } else {
          setRecoveries([]);
          setInstallHistory([]);
        }
      } catch (err) {
        if (alive) {
          setError(getUpdateErrorMessage(err));
        }
      } finally {
        if (alive) {
          setHydratedKey(hydrationKey);
        }
      }
    };

    void hydrate();

    return () => {
      alive = false;
      currentClientIdRef.current = null;
    };
  }, [hydrationKey, smokeClientInstallDir, smokeEnabled, smokeManifestUrl, tauriRuntime]);

  useEffect(() => {
    if (!tauriRuntime) {
      return;
    }

    let disposed = false;
    let cleanupDownloadProgress: UnlistenFn | undefined;
    let cleanupDownloadCompleted: UnlistenFn | undefined;
    let cleanupDownloadFailed: UnlistenFn | undefined;
    let cleanupInstallProgress: UnlistenFn | undefined;
    let cleanupInstallCompleted: UnlistenFn | undefined;
    let cleanupInstallFailed: UnlistenFn | undefined;

    const refreshAfterEvent = () => {
      void refreshClientArtifacts().catch((err) => {
        if (!disposed) {
          setError(getUpdateErrorMessage(err));
        }
      });
    };
    const isCurrentClientJob = (job: DownloadJob) => job.client_installation_id === currentClientIdRef.current;

    void listen<DownloadJob>("download-progress", (event) => {
      if (!isCurrentClientJob(event.payload)) {
        return;
      }
      setJob(event.payload);
      setNotice(null);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupDownloadProgress = fn;
    });

    void listen<DownloadJob>("download-completed", (event) => {
      if (!isCurrentClientJob(event.payload)) {
        return;
      }
      setJob(event.payload);
      setError(null);
      setNotice("已校验");
      if (smokeAutomation) {
        smokePhaseRef.current = "waiting_install";
      }
      refreshAfterEvent();
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupDownloadCompleted = fn;
    });

    void listen<DownloadJob>("download-failed", (event) => {
      if (!isCurrentClientJob(event.payload)) {
        return;
      }
      setJob(event.payload);
      setNotice(null);
      if (smokeAutomation) {
        smokeFailureStageRef.current = smokePhaseRef.current === "downloading" ? "download" : smokeFailureStageRef.current;
        void completeSmoke("failed", smokeFailureStageRef.current, event.payload.error ?? "download_failed");
      }
      if (event.payload.error) {
        setError(getUpdateErrorMessage(event.payload.error));
      }
      refreshAfterEvent();
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupDownloadFailed = fn;
    });

    void listen<string>("install-progress", (event) => {
      setJob((current) => {
        if (!current || current.id !== event.payload) {
          return current;
        }
        if (smokeAutomation) {
          smokePhaseRef.current = "installing";
        }
        setNotice("安装中");
        return {
          ...current,
          status: "installing"
        };
      });
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupInstallProgress = fn;
    });

    void listen<DownloadJob>("install-completed", (event) => {
      if (!isCurrentClientJob(event.payload)) {
        return;
      }
      setJob(event.payload);
      setError(null);
      setNotice("已完成");
      if (smokeAutomation) {
        void completeSmoke("succeeded", "install", null);
      }
      refreshAfterEvent();
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupInstallCompleted = fn;
    });

    void listen<DownloadJob>("install-failed", (event) => {
      if (!isCurrentClientJob(event.payload)) {
        return;
      }
      setJob(event.payload);
      setNotice(null);
      if (smokeAutomation) {
        smokeFailureStageRef.current = "install";
        void completeSmoke("failed", "install", event.payload.error ?? "install_failed");
      }
      setError(event.payload.error ? getUpdateErrorMessage(event.payload.error) : "操作失败，请稍后重试。");
      refreshAfterEvent();
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupInstallFailed = fn;
    });

    return () => {
      disposed = true;
      cleanupDownloadProgress?.();
      cleanupDownloadCompleted?.();
      cleanupDownloadFailed?.();
      cleanupInstallProgress?.();
      cleanupInstallCompleted?.();
      cleanupInstallFailed?.();
    };
  }, [completeSmoke, refreshClientArtifacts, smokeAutomation, smokeEnabled, tauriRuntime]);

  const resetResult = () => {
    latestRequestIdRef.current += 1;
    setUpdate(null);
    setJob(null);
    setError(null);
    setNotice(null);
  };

  const check = useCallback(async () => {
    if (!client) {
      const message = "未设置默认客户端";
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "bootstrap", message);
      }
      return;
    }
    if (activeUseManifestSource && !activeManifestUrl.trim()) {
      const message = "未填写 Manifest 地址";
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "bootstrap", message);
      }
      return;
    }

    const requestId = latestRequestIdRef.current + 1;
    latestRequestIdRef.current = requestId;
    if (smokeAutomation) {
      smokeFailureStageRef.current = "check";
      smokePhaseRef.current = "checking";
    }
    setError(null);
    setNotice(null);
    setUpdate(null);
    setJob(null);
    setIsBusy(true);

    try {
      const result = await checkClientUpdate(buildUpdateSourceRequest({
        clientId: client.client_id,
        channel,
        manifestUrl: activeManifestUrl,
        routeMode: activeRouteMode,
        routeUrl: activeRouteUrl,
        useManifestSource: activeUseManifestSource
      }));
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setUpdate(result);
      if (!result) {
        const message = "无可用更新";
        setError(message);
        if (smokeAutomation) {
          void completeSmoke("failed", "check", message);
        }
        return;
      }
      if (smokeAutomation) {
        if (!result.needs_update) {
          void completeSmoke("failed", "check", result.message ?? "已最新");
          return;
        }
        if (result.action !== "download") {
          void completeSmoke("failed", "check", result.message ?? "该更新来源不提供自动下载。");
          return;
        }
        smokePhaseRef.current = "waiting_download";
      }
    } catch (err) {
      if (latestRequestIdRef.current === requestId) {
        const message = err instanceof Error && err.message === "route_url_invalid" ? "请输入有效的网络地址。" : getUpdateErrorMessage(err);
        setError(message);
        if (smokeAutomation) {
          void completeSmoke("failed", "check", message);
        }
      }
    } finally {
      if (latestRequestIdRef.current === requestId) {
        setIsBusy(false);
      }
    }
  }, [
    activeManifestUrl,
    activeUseManifestSource,
    channel,
    client,
    completeSmoke,
    activeRouteMode,
    activeRouteUrl,
    setError,
    setIsBusy,
    setJob,
    setNotice,
    setUpdate,
    smokeAutomation
  ]);

  const download = useCallback(async () => {
    if (!client || !update) {
      return;
    }
    if (update.action !== "download") {
      const message = update.message ?? "该更新来源不提供自动下载。";
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "download", message);
      }
      return;
    }
    if (activeUseManifestSource && !activeManifestUrl.trim()) {
      const message = "未填写 Manifest 地址";
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "bootstrap", message);
      }
      return;
    }

    if (smokeAutomation) {
      smokeFailureStageRef.current = "download";
      smokePhaseRef.current = "downloading";
    }
    setError(null);
    setNotice(null);
    setIsBusy(true);
    try {
      const nextJob = await startUpdateDownload(buildStartUpdateDownloadRequest({
        clientInstallationId: client.id,
        channel: update.channel,
        manifestUrl: activeManifestUrl,
        routeMode: activeRouteMode,
        routeUrl: activeRouteUrl,
        useManifestSource: activeUseManifestSource
      }));
      setJob(nextJob);
      if (smokeAutomation) {
        if (nextJob.status === "verified") {
          smokePhaseRef.current = "waiting_install";
        } else if (nextJob.status === "failed") {
          void completeSmoke("failed", "download", nextJob.error ?? "download_failed");
        }
      }
      await refreshClientArtifacts();
    } catch (err) {
      const message = err instanceof Error && err.message === "route_url_invalid" ? "请输入有效的网络地址。" : getUpdateErrorMessage(err);
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "download", message);
      }
    } finally {
      setIsBusy(false);
    }
  }, [
    activeManifestUrl,
    activeUseManifestSource,
    client,
    completeSmoke,
    refreshClientArtifacts,
    activeRouteMode,
    activeRouteUrl,
    setError,
    setIsBusy,
    setJob,
    setNotice,
    smokeAutomation,
    update
  ]);

  const installJob = useCallback(async (jobId: string, optimisticJob?: DownloadJob) => {
    if (optimisticJob) {
      setJob(optimisticJob);
    }

    if (smokeAutomation) {
      smokeFailureStageRef.current = "install";
      smokePhaseRef.current = "installing";
    }
    setError(null);
    setNotice("准备安装");
    setIsBusy(true);
    try {
      const nextJob = await installDownloadedUpdate(jobId);
      setJob(nextJob);
      await refreshClientArtifacts();
      if (nextJob.status === "failed") {
        setNotice(null);
        const message = nextJob.error ? getUpdateErrorMessage(nextJob.error) : "操作失败，请稍后重试。";
        setError(message);
        if (smokeAutomation) {
          void completeSmoke("failed", "install", nextJob.error ?? message);
        }
        return;
      }
      setNotice("已完成");
      if (smokeAutomation && nextJob.status === "completed") {
        void completeSmoke("succeeded", "install", null);
      }
    } catch (err) {
      setNotice(null);
      const message = getUpdateErrorMessage(err);
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "install", message);
      }
    } finally {
      setIsBusy(false);
    }
  }, [completeSmoke, refreshClientArtifacts, setError, setIsBusy, setJob, setNotice, smokeAutomation]);

  useEffect(() => {
    if (!smokeEnabled || hydratedKey !== hydrationKey || isBusy || smokeReportedRef.current) {
      return;
    }

    if (!client) {
      void completeSmoke("failed", "bootstrap", "未设置默认客户端");
      return;
    }

    if (smokePhaseRef.current === "idle") {
      void check();
      return;
    }

    if (smokePhaseRef.current === "waiting_download" && update?.needs_update && update.action === "download") {
      void download();
      return;
    }

    if (smokePhaseRef.current === "waiting_install" && job?.status === "verified") {
      void installJob(job.id);
    }
  }, [check, client, completeSmoke, download, hydratedKey, hydrationKey, installJob, isBusy, job, smokeEnabled, update]);

  const percent = progressPercent(job);
  const visibleClient = tauriRuntime ? client : null;
  const visibleRecoveries = (tauriRuntime ? recoveries : []).filter(
    (recovery) => recovery.can_install || recovery.can_retry || recovery.job.status !== "completed"
  );
  const visibleInstallHistory = tauriRuntime ? installHistory : [];
  const visibleError = tauriRuntime ? error : "浏览器预览";
  const visibleNotice = tauriRuntime ? notice : null;

  return (
    <div className="space-y-5">
      <div className="space-y-3">
        <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
          <span className="text-[var(--app-text-muted)] text-xs font-bold uppercase tracking-wider">更新源</span>
        </div>
        <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 space-y-3.5">
          <div className="flex items-center justify-between py-1">
            <span className="text-xs font-medium text-[var(--app-text-secondary)]">类型</span>
            <span className="text-xs font-bold text-[var(--app-text)]">
              {props.settings.advanced_manifest_url !== null ? "自定义更新配置文件" : "内置客户端更新源"}
            </span>
          </div>
          <div className="border-t border-[var(--app-border-subtle)]" />
          <div className="flex items-center justify-between py-1">
            <label className="text-xs font-medium text-[var(--app-text-secondary)]" htmlFor="channel-select">更新渠道</label>
            <select
              id="channel-select"
              value={channel}
              onChange={(event) => { resetResult(); setChannel(event.target.value); }}
              disabled={isBusy}
              className="bg-[var(--app-surface)] border border-[var(--app-border-subtle)] rounded-lg px-2 py-1 text-xs text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] font-mono transition-colors cursor-pointer w-32"
            >
              <option value="stable">stable (稳定版)</option>
              <option value="nightly">nightly (测试版)</option>
            </select>
          </div>
        </div>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
          <span className="text-[var(--app-text-muted)] text-xs font-bold uppercase tracking-wider">自定义更新配置</span>
        </div>
        <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 space-y-3">
          <div className="flex items-start justify-between py-1">
            <div className="space-y-1">
              <span className="text-xs font-medium text-[var(--app-text-secondary)] block">使用自定义更新源 (高级)</span>
              <span className="text-[10px] text-[var(--app-text-dim)] block leading-relaxed max-w-[280px]">
                启用后将使用自定义的 Manifest JSON 配置文件作为客户端更新源，通常用于第三方或开发版客户端。
              </span>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={props.settings.advanced_manifest_url !== null}
              onClick={() => {
                resetResult();
                const isCurrentlyActive = props.settings.advanced_manifest_url !== null;
                void props.onUpdateSettings({
                  ...props.settings,
                  advanced_manifest_url: isCurrentlyActive ? null : ""
                });
              }}
              disabled={isBusy}
              className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer disabled:cursor-not-allowed disabled:opacity-55 ${
                props.settings.advanced_manifest_url !== null ? "bg-[var(--app-accent)]" : "bg-[var(--app-surface)] border border-[var(--app-border-subtle)]"
              }`}
            >
              <div
                className={`w-[14px] h-[14px] rounded-full transition-transform bg-white shadow-sm ${
                  props.settings.advanced_manifest_url !== null ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </div>
          {props.settings.advanced_manifest_url !== null ? (
            <div className="pt-2 border-t border-[var(--app-border-subtle)]">
              <input
                id="manifest-url-input"
                aria-label="自定义 manifest 地址"
                value={props.settings.advanced_manifest_url}
                onChange={(event) => {
                  resetResult();
                  void props.onUpdateSettings({
                    ...props.settings,
                    advanced_manifest_url: event.target.value
                  });
                }}
                disabled={isBusy}
                className="bg-[var(--app-surface)] border border-[var(--app-border-subtle)] rounded-lg px-3.5 py-2 w-full text-xs text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] font-mono transition-colors"
                placeholder="https://gitee.com/example/manifest/raw/main/ddnet.json"
                spellCheck={false}
              />
            </div>
          ) : null}
        </div>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
          <span className="text-[var(--app-text-muted)] text-xs font-bold uppercase tracking-wider">网络路由</span>
        </div>
        <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 space-y-3.5">
          <div className="flex flex-wrap gap-2">
            {(["direct", "proxy_prefix", "mirror_template"] as const).map((mode) => {
              const active = activeRouteMode === mode;
              return (
                <button
                  key={mode}
                  type="button"
                  onClick={() => {
                    resetResult();
                    void props.onUpdateSettings(updateNetworkRoute(props.settings, mode, activeRouteUrl));
                  }}
                  disabled={isBusy}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold tracking-wide transition-all cursor-pointer disabled:cursor-not-allowed disabled:opacity-55 ${
                    active
                      ? "bg-[var(--app-border-strong)] text-[var(--app-text)] shadow-sm border border-[var(--app-border-subtle)]"
                      : "text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] hover:bg-black/20"
                  }`}
                >
                  {networkRouteLabel(mode)}
                </button>
              );
            })}
          </div>
          {activeRouteMode !== "direct" ? (
            <div className="pt-2 border-t border-[var(--app-border-subtle)]">
              <input
                aria-label={activeRouteMode === "proxy_prefix" ? "代理前缀地址" : "镜像模板地址"}
                value={activeRouteUrl}
                onChange={(event) => {
                  resetResult();
                  void props.onUpdateSettings(updateNetworkRoute(props.settings, activeRouteMode, event.target.value));
                }}
                disabled={isBusy}
                className="bg-[var(--app-surface)] border border-[var(--app-border-subtle)] rounded-lg px-3.5 py-2 w-full text-xs text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] font-mono transition-colors"
                placeholder={activeRouteMode === "proxy_prefix" ? "填写你的代理前缀地址 (如 https://proxy.example/)" : "填写包含 {url} 的镜像模板"}
                spellCheck={false}
              />
            </div>
          ) : null}
        </div>
      </div>

      <div className="grid gap-4 xl:grid-cols-[260px_minmax(0,1fr)]">
        <div className="space-y-3">
          <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
            <span className="text-[var(--app-text-muted)] text-xs font-bold uppercase tracking-wider">默认客户端</span>
          </div>
          <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4">
            <div className="text-sm font-bold text-[var(--app-text)]">{visibleClient?.display_name ?? "未设置"}</div>
            <div className="mt-1 break-all text-[10px] text-[var(--app-text-dim)] font-mono">{visibleClient?.install_dir ?? "未设置"}</div>
            <div className="mt-2 text-[10px] font-bold text-[var(--app-accent)]">
              {visibleClient ? `当前版本：${visibleClient.version ?? "未知"}` : "-"}
            </div>
          </div>
          <button
            type="button"
            onClick={() => void check()}
            disabled={!visibleClient || isBusy}
            className="w-full h-10 rounded-lg bg-[var(--app-accent)] text-black hover:bg-cyan-400 text-xs font-bold cursor-pointer transition-all disabled:cursor-not-allowed disabled:opacity-45"
          >
            {isBusy ? "请稍候..." : "检查更新"}
          </button>
        </div>

        <div className="space-y-3">
          <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
            <span className="text-[var(--app-text-muted)] text-xs font-bold uppercase tracking-wider">可用更新</span>
          </div>
          {update?.action === "open_url" ? (
            <div className="mt-3 rounded-xl bg-[var(--app-input)] border border-[var(--app-border-subtle)] p-4">
              <div className="flex flex-wrap items-center gap-2 text-xs font-black text-[var(--app-text-muted)]">
                <span className="rounded-full bg-black/20 px-3 py-1">{updateSourceLabel(update.source_kind)}</span>
                {update.latest_version ? (
                  <span className="rounded-full bg-black/20 px-3 py-1">{update.latest_version}</span>
                ) : null}
              </div>
              <div className="mt-3 text-sm font-bold leading-7 text-[var(--app-text-muted)]">
                {update.message ?? "该更新来源需要打开上游页面手动处理。"}
              </div>
              {update.action_url ? (
                <button
                  type="button"
                  onClick={() => {
                    if (update.action_url) {
                      window.open(update.action_url, "_blank", "noreferrer");
                    }
                  }}
                  className="mt-4 inline-flex h-10 items-center rounded-lg bg-[var(--app-accent)] px-4 text-xs font-bold text-black hover:bg-cyan-400 transition-all cursor-pointer"
                >
                  打开上游页面
                </button>
              ) : null}
            </div>
          ) : update ? (
            <div className="mt-3 rounded-xl bg-[var(--app-input)] border border-[var(--app-border-subtle)] p-4">
              <div className="flex flex-wrap items-center gap-2 text-xs font-black text-[var(--app-text-muted)]">
                <span className="rounded-full bg-black/20 px-3 py-1">{update.channel}</span>
                <span className="rounded-full bg-black/20 px-3 py-1">{update.latest_version}</span>
                <span className="rounded-full bg-black/20 px-3 py-1">{update.asset.platform}</span>
                <span className="rounded-full bg-black/20 px-3 py-1">{formatAssetSize(update.asset.size)}</span>
              </div>
              <div className="mt-3 text-xs font-black text-[var(--app-text-muted)]">
                {update.needs_update ? "可更新" : "已最新"}
              </div>
              <button
                type="button"
                onClick={() => void download()}
                disabled={!update.needs_update || isBusy}
                className="mt-4 h-10 rounded-lg bg-[var(--app-accent)] px-4 text-xs font-bold text-black hover:bg-cyan-400 transition-all cursor-pointer disabled:cursor-not-allowed disabled:opacity-45"
              >
                开始下载
              </button>
            </div>
          ) : (
            <div className="mt-3 rounded-xl border border-dashed border-[var(--app-border-subtle)] bg-black/25 px-4 py-6 text-sm font-semibold text-[var(--app-text-muted)] text-center">
              空
            </div>
          )}

          {job ? (
            <div className="mt-4 rounded-xl bg-[var(--app-input)] border border-[var(--app-border-subtle)] p-4">
              <div className="flex items-center justify-between text-xs font-black text-[var(--app-text-muted)]">
                <span>{downloadStatusLabel(job.status)}</span>
                <span>{percent}%</span>
              </div>
              <div className="mt-3 h-2 overflow-hidden rounded-full bg-black/20">
                <div className="h-full bg-[var(--app-accent)] transition-all" style={{ width: `${percent}%` }} />
              </div>
              <div className="mt-3 text-xs leading-6 text-[var(--app-text-muted)]">
                {job.status === "verified"
                  ? "已校验"
                  : job.status === "installing"
                    ? "安装中"
                    : "已下载"}
              </div>
              {job.error ? <div className="mt-2 text-xs font-bold text-red-400">{getUpdateErrorMessage(job.error)}</div> : null}
              <button
                type="button"
                onClick={() => void installJob(job.id)}
                disabled={job.status !== "verified" || isBusy}
                className="mt-4 h-10 rounded-lg bg-[var(--app-accent)] px-4 text-xs font-bold text-black hover:bg-cyan-400 transition-all cursor-pointer disabled:cursor-not-allowed disabled:opacity-45"
              >
                安装更新
              </button>
            </div>
          ) : null}
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-2">
        <div className="rounded-xl bg-black/20 p-4 border border-[var(--app-border-subtle)]">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-black text-[var(--app-text-muted)]">恢复任务</div>
            </div>
            <span className="rounded-full bg-[var(--app-accent)] px-3 py-1 text-[11px] font-black text-black">{visibleRecoveries.length}</span>
          </div>
          {visibleRecoveries.length > 0 ? (
            <div className="mt-3 grid gap-3">
              {visibleRecoveries.map((recovery) => (
                <div key={recovery.job.id} className="rounded-xl bg-[var(--app-input)] border border-[var(--app-border-subtle)] p-4">
                  <div className="flex flex-wrap items-center gap-2 text-[11px] font-black text-[var(--app-text-muted)]">
                    <span className="rounded-full bg-black/20 px-3 py-1">{recovery.job.version}</span>
                    <span className="rounded-full bg-black/20 px-3 py-1">{downloadStatusLabel(recovery.job.status)}</span>
                    <span className="rounded-full bg-black/20 px-3 py-1">{recoveryStateLabel(recovery.cache_state)}</span>
                  </div>
                  <div className="mt-3 text-sm font-bold leading-6 text-[var(--app-text)]">{recovery.user_message}</div>
                  <div className="mt-2 break-all text-xs font-semibold text-[var(--app-text-dim)]">{recovery.job.cache_path}</div>
                  <div className="mt-3 flex flex-wrap items-center gap-3 text-[11px] font-black text-[var(--app-text-muted)]">
                    <span>已下载 {formatAssetSize(recovery.job.downloaded_bytes)} / {formatAssetSize(recovery.job.size)}</span>
                    <span>{recovery.can_retry ? "建议重新下载" : "无需重下"}</span>
                  </div>
                  {recovery.can_install ? (
                    <button
                      type="button"
                      onClick={() => void installJob(recovery.job.id, recovery.job)}
                      disabled={isBusy}
                      className="mt-4 h-10 rounded-lg bg-[var(--app-accent)] px-4 text-xs font-bold text-black hover:bg-cyan-400 transition-all cursor-pointer disabled:cursor-not-allowed disabled:opacity-45"
                    >
                      继续安装
                    </button>
                  ) : null}
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-3 rounded-xl border border-dashed border-[var(--app-border-subtle)] bg-black/25 px-4 py-6 text-sm font-semibold text-[var(--app-text-muted)] text-center">
              空
            </div>
          )}
        </div>

        <div className="rounded-xl bg-black/20 p-4 border border-[var(--app-border-subtle)]">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-black text-[var(--app-text-muted)]">安装历史</div>
            </div>
            <span className="rounded-full bg-[var(--app-accent)] px-3 py-1 text-[11px] font-black text-black">{visibleInstallHistory.length}</span>
          </div>
          {visibleInstallHistory.length > 0 ? (
            <div className="mt-3 grid gap-3">
              {visibleInstallHistory.map((record) => (
                <div key={record.id} className="rounded-xl bg-[var(--app-input)] border border-[var(--app-border-subtle)] p-4">
                  <div className="flex flex-wrap items-center gap-2 text-[11px] font-black text-[var(--app-text-muted)]">
                    <span className="rounded-full bg-black/20 px-3 py-1">{record.version}</span>
                    <span className={`rounded-full px-3 py-1 border ${record.status === "completed" ? "bg-emerald-500/15 text-emerald-400 border-emerald-500/30" : "bg-red-500/15 text-red-400 border-red-500/30"}`}>
                      {installHistoryStatusLabel(record.status)}
                    </span>
                    <span className="rounded-full bg-black/20 px-3 py-1">{record.package_kind}</span>
                  </div>
                  <div className="mt-3 text-xs font-bold text-[var(--app-text-muted)]">完成时间：{formatCompletedAt(record.completed_at)}</div>
                  <div className="mt-2 break-all text-xs font-semibold text-[var(--app-text-dim)]">
                    回滚目录：{record.rollback_path ?? "未记录"}
                  </div>
                  {record.error ? <div className="mt-2 text-xs font-bold text-red-400">{getUpdateErrorMessage(record.error)}</div> : null}
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-3 rounded-xl border border-dashed border-[var(--app-border-subtle)] bg-black/25 px-4 py-6 text-sm font-semibold text-[var(--app-text-muted)] text-center">
              空
            </div>
          )}
        </div>
      </div>

      {visibleNotice ? <div className="rounded-xl border border-yellow-400/20 bg-[var(--app-accent)]/5 px-3 py-2 text-xs font-semibold text-[var(--app-accent)]">{visibleNotice}</div> : null}
      {visibleError ? <div className="rounded-xl border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs font-semibold text-red-400">{visibleError}</div> : null}
    </div>
  );
}
