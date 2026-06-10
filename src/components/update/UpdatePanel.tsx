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
  loadAppSettings,
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
  NetworkRouteMode
} from "../../types";
import { getUpdateErrorMessage } from "../../lib/errors";
import {
  buildStartUpdateDownloadRequest,
  buildUpdateSourceRequest,
  networkRouteLabel,
  progressPercent,
  resolveUpdateManifestInput
} from "../../lib/updateLogic";

const MANIFEST_URL_PLACEHOLDER = "自维护 manifest / 调试用途";

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

export function UpdatePanel(props: { smokeAutomation?: LocalSmokeAutomationConfig | null }) {
  const tauriRuntime = isTauriRuntime();
  const smokeAutomation = props.smokeAutomation ?? null;
  const smokeEnabled = smokeAutomation !== null;
  const smokeClientInstallDir = smokeAutomation?.clientInstallDir.trim() ?? "";
  const smokeManifestUrl = smokeAutomation?.manifestUrl.trim() ?? "";
  const smokeCloseWindowOnFinish = smokeAutomation?.closeWindowOnFinish ?? false;
  const [hydratedKey, setHydratedKey] = useState<string | null>(null);
  const [manifestUrl, setManifestUrl] = useState("");
  const [useManifestSource, setUseManifestSource] = useState(false);
  const [channel, setChannel] = useState("stable");
  const [routeMode, setRouteMode] = useState<NetworkRouteMode>("direct");
  const [routeUrl, setRouteUrl] = useState("");
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
  const manifestInput = resolveUpdateManifestInput({
    smokeEnabled,
    smokeManifestUrl,
    useManifestSource,
    manifestUrl
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
        const [storedClient, settings] = await Promise.all([
          smokeEnabled ? Promise.resolve(null) : getDefaultClient(),
          loadAppSettings()
        ]);
        if (!alive) {
          return;
        }

        const nextClient = smokeEnabled
          ? await upsertClientInstallation({
              install_dir: (await validateClientDir(smokeClientInstallDir)).install_dir,
              is_default: false
            })
          : storedClient;
        if (!alive) {
          return;
        }

        setClient(nextClient);
        currentClientIdRef.current = nextClient?.id ?? null;
        if (smokeEnabled) {
          setManifestUrl(smokeManifestUrl);
        } else if (settings.advanced_manifest_url) {
          setManifestUrl(settings.advanced_manifest_url);
        }
        if (settings.network_route) {
          setRouteMode(settings.network_route.mode);
          setRouteUrl(settings.network_route.proxy_prefix_url ?? settings.network_route.mirror_template ?? "");
        }
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
      setNotice("下载完成，文件已通过校验，可以直接安装。");
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
        setNotice("正在安装更新，请保持客户端关闭。");
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
      setNotice("安装已完成，客户端版本记录已刷新。");
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
      const message = "请先在客户端管理中保存默认客户端。";
      setError(message);
      if (smokeAutomation) {
        void completeSmoke("failed", "bootstrap", message);
      }
      return;
    }
    if (activeUseManifestSource && !activeManifestUrl.trim()) {
      const message = "请先填写自维护 manifest 地址。";
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
        routeMode,
        routeUrl,
        useManifestSource: activeUseManifestSource
      }));
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setUpdate(result);
      if (!result) {
        const message = "更新源里没有适合当前客户端的版本。";
        setError(message);
        if (smokeAutomation) {
          void completeSmoke("failed", "check", message);
        }
        return;
      }
      if (smokeAutomation) {
        if (!result.needs_update) {
          void completeSmoke("failed", "check", result.message ?? "当前已是最新版本。");
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
    routeMode,
    routeUrl,
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
      const message = "请先填写自维护 manifest 地址。";
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
        routeMode,
        routeUrl,
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
    routeMode,
    routeUrl,
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
    setNotice("正在准备安装，请保持客户端关闭。");
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
      setNotice("安装已完成，客户端版本记录已刷新。");
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
      void completeSmoke("failed", "bootstrap", "请先在客户端管理中保存默认客户端。");
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
  const visibleError = tauriRuntime ? error : "当前为浏览器预览模式，更新能力需在 Tauri 桌面运行时验证。";
  const visibleNotice = tauriRuntime ? notice : null;

  return (
    <section className="rounded-[28px] border border-[var(--dm-border)] bg-[#161719]/65 p-6 text-[var(--dm-ink)] shadow-[0_24px_70px_rgba(0,0,0,0.4)] backdrop-blur-2xl">
      <h2 className="text-3xl font-black tracking-[0]">客户端更新</h2>
      <p className="mt-2 text-sm font-semibold text-[var(--dm-muted-ink)]">检查默认客户端是否有新版本，下载完成后会先校验文件，再安装。</p>

      <div className="mt-5 grid gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="text-[11px] font-black tracking-[0.18em] text-[var(--dm-muted-ink)]">更新源类型</div>
          <div className="mt-3 rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
            <div className="text-sm font-black text-[var(--dm-ink)]">
              {activeUseManifestSource ? "ManifestSource" : "内置客户端更新源"}
            </div>
            <div className="mt-2 text-xs font-bold leading-6 text-[var(--dm-muted-ink)]">
              普通流程会根据客户端类型自动选择 GitHub Release、官网或手动下载入口。
            </div>
          </div>
        </div>
        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <label className="block text-[11px] font-black tracking-[0.18em] text-[var(--dm-muted-ink)]" htmlFor="channel-input">
            更新渠道
          </label>
          <input
            id="channel-input"
            value={channel}
            onChange={(event) => {
              resetResult();
              setChannel(event.target.value);
            }}
            disabled={isBusy}
            className="mt-3 h-12 w-full rounded-[16px] border border-[var(--dm-border)] bg-black/30 px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#5f6673] focus:border-[#41f2ff]/40 focus:ring-4 focus:ring-[#41f2ff]/10"
            spellCheck={false}
          />
        </div>
      </div>

      <div className="mt-4 rounded-[22px] bg-[var(--dm-soft)] p-4">
        <button
          type="button"
          onClick={() => {
            resetResult();
            setUseManifestSource((value) => !value);
          }}
          disabled={isBusy}
          className="flex w-full items-center justify-between text-left text-xs font-black text-[var(--dm-muted-ink)] disabled:cursor-not-allowed disabled:opacity-55"
        >
          <span>自维护 manifest / 调试用途</span>
          <span>{activeUseManifestSource ? "已启用" : "默认折叠"}</span>
        </button>
        {activeUseManifestSource ? (
          <input
            id="manifest-url-input"
            aria-label="自维护 manifest 地址"
            value={activeManifestUrl}
            onChange={(event) => {
              resetResult();
              setManifestUrl(event.target.value);
            }}
            disabled={isBusy}
            className="mt-3 h-12 w-full rounded-[16px] border border-[var(--dm-border)] bg-black/30 px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#5f6673] focus:border-[#41f2ff]/40 focus:ring-4 focus:ring-[#41f2ff]/10"
            placeholder={MANIFEST_URL_PLACEHOLDER}
            spellCheck={false}
          />
        ) : null}
      </div>

      <div className="mt-4 rounded-[22px] bg-[var(--dm-soft)] p-4">
        <div className="text-xs font-black text-[var(--dm-muted-ink)]">下载网络</div>
        <div className="mt-3 flex flex-wrap gap-2">
          {(["direct", "proxy_prefix", "mirror_template"] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              onClick={() => {
                resetResult();
                setRouteMode(mode);
              }}
              disabled={isBusy}
              className={`h-10 rounded-[15px] border px-4 text-xs font-black transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55 ${
                routeMode === mode
                  ? "border-[#41f2ff] bg-[#41f2ff] text-[#111213] shadow-[0_0_10px_rgba(65,242,255,0.2)]"
                  : "border-[var(--dm-border)] bg-black/30 text-[var(--dm-muted-ink)]"
              }`}
            >
              {networkRouteLabel(mode)}
            </button>
          ))}
        </div>
        {routeMode !== "direct" ? (
          <input
            aria-label={routeMode === "proxy_prefix" ? "代理前缀地址" : "镜像模板地址"}
            value={routeUrl}
            onChange={(event) => {
              resetResult();
              setRouteUrl(event.target.value);
            }}
            disabled={isBusy}
            className="mt-3 h-12 w-full rounded-[16px] border border-[var(--dm-border)] bg-black/30 px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#5f6673] focus:border-[#41f2ff]/40 focus:ring-4 focus:ring-[#41f2ff]/10"
            placeholder={routeMode === "proxy_prefix" ? "填写你的代理前缀地址" : "填写包含 {url} 的镜像模板"}
            spellCheck={false}
          />
        ) : null}
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[280px_minmax(0,1fr)]">
        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="text-xs font-black text-[var(--dm-muted-ink)]">默认客户端</div>
          <div className="mt-3 rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
            <div className="text-lg font-black">{visibleClient?.display_name ?? "未设置"}</div>
            <div className="mt-2 break-all text-xs font-bold text-[var(--dm-muted-ink)]">{visibleClient?.install_dir ?? "请先保存默认客户端"}</div>
            <div className="mt-3 text-xs font-black text-[#41f2ff]">
              {visibleClient ? `当前版本：${visibleClient.version ?? "未知"}` : "-"}
            </div>
          </div>
          <button
            type="button"
            onClick={() => void check()}
            disabled={!visibleClient || isBusy}
            className="mt-4 h-11 w-full rounded-[16px] bg-[#41f2ff] px-5 text-sm font-black text-[#111213] shadow-[0_0_15px_rgba(65,242,255,0.25)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
          >
            {isBusy ? "请稍候" : "检查更新"}
          </button>
        </div>

        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="text-xs font-black text-[var(--dm-muted-ink)]">可用更新</div>
          {update?.action === "open_url" ? (
            <div className="mt-3 rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
              <div className="flex flex-wrap items-center gap-2 text-xs font-black text-[var(--dm-muted-ink)]">
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{updateSourceLabel(update.source_kind)}</span>
                {update.latest_version ? (
                  <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.latest_version}</span>
                ) : null}
              </div>
              <div className="mt-3 text-sm font-bold leading-7 text-[var(--dm-muted-ink)]">
                {update.message ?? "该更新来源需要打开上游页面手动处理。"}
              </div>
              {update.action_url ? (
                <a
                  href={update.action_url}
                  target="_blank"
                  rel="noreferrer"
                  className="mt-4 inline-flex h-10 items-center rounded-[15px] bg-[#41f2ff] px-4 text-sm font-black text-[#111213] shadow-[0_0_15px_rgba(65,242,255,0.25)] transition hover:-translate-y-0.5"
                >
                  打开上游页面
                </a>
              ) : null}
            </div>
          ) : update ? (
            <div className="mt-3 rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
              <div className="flex flex-wrap items-center gap-2 text-xs font-black text-[var(--dm-muted-ink)]">
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.channel}</span>
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.latest_version}</span>
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.asset.platform}</span>
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{formatAssetSize(update.asset.size)}</span>
              </div>
              <div className="mt-3 text-xs font-black text-[var(--dm-muted-ink)]">
                {update.needs_update ? "需要更新" : "当前已是最新版本"}
              </div>
              <button
                type="button"
                onClick={() => void download()}
                disabled={!update.needs_update || isBusy}
                className="mt-4 h-10 rounded-[15px] bg-[#41f2ff] px-4 text-sm font-black text-[#111213] shadow-[0_0_15px_rgba(65,242,255,0.25)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
              >
                开始下载
              </button>
            </div>
          ) : (
            <div className="mt-3 rounded-[18px] border border-dashed border-[var(--dm-border)] bg-black/25 px-4 py-6 text-sm font-semibold text-[var(--dm-muted-ink)]">
              检查更新后会显示可下载的版本。
            </div>
          )}

          {job ? (
            <div className="mt-4 rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
              <div className="flex items-center justify-between text-xs font-black text-[var(--dm-muted-ink)]">
                <span>{downloadStatusLabel(job.status)}</span>
                <span>{percent}%</span>
              </div>
              <div className="mt-3 h-2 overflow-hidden rounded-full bg-[var(--dm-soft)]">
                <div className="h-full bg-[#41f2ff] shadow-[0_0_8px_#41f2ff] transition-all" style={{ width: `${percent}%` }} />
              </div>
              <div className="mt-3 text-xs leading-6 text-[var(--dm-muted-ink)]">
                {job.status === "verified"
                  ? "下载完成，文件已通过校验。"
                  : job.status === "installing"
                    ? "正在安装更新，请保持客户端关闭。"
                    : "下载文件会保存在应用缓存中。"}
              </div>
              {job.error ? <div className="mt-2 text-xs font-bold text-red-400">{getUpdateErrorMessage(job.error)}</div> : null}
              <button
                type="button"
                onClick={() => void installJob(job.id)}
                disabled={job.status !== "verified" || isBusy}
                className="mt-4 h-10 rounded-[15px] bg-[#41f2ff] px-4 text-sm font-black text-[#111213] shadow-[0_0_10px_rgba(65,242,255,0.25)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-45"
              >
                安装更新
              </button>
            </div>
          ) : null}
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-2">
        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-black text-[var(--dm-muted-ink)]">恢复任务</div>
              <div className="mt-1 text-[11px] font-bold text-slate-500">展示当前默认客户端可继续安装或可重试的下载记录。</div>
            </div>
            <span className="rounded-full bg-[#41f2ff] px-3 py-1 text-[11px] font-black text-[#111213] shadow-[0_0_10px_rgba(65,242,255,0.3)]">{visibleRecoveries.length}</span>
          </div>
          {visibleRecoveries.length > 0 ? (
            <div className="mt-3 grid gap-3">
              {visibleRecoveries.map((recovery) => (
                <div key={recovery.job.id} className="rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
                  <div className="flex flex-wrap items-center gap-2 text-[11px] font-black text-[var(--dm-muted-ink)]">
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{recovery.job.version}</span>
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{downloadStatusLabel(recovery.job.status)}</span>
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{recoveryStateLabel(recovery.cache_state)}</span>
                  </div>
                  <div className="mt-3 text-sm font-bold leading-6 text-[var(--dm-ink)]">{recovery.user_message}</div>
                  <div className="mt-2 break-all text-xs font-semibold text-slate-500">{recovery.job.cache_path}</div>
                  <div className="mt-3 flex flex-wrap items-center gap-3 text-[11px] font-black text-[var(--dm-muted-ink)]">
                    <span>已下载 {formatAssetSize(recovery.job.downloaded_bytes)} / {formatAssetSize(recovery.job.size)}</span>
                    <span>{recovery.can_retry ? "建议重新下载" : "无需重下"}</span>
                  </div>
                  {recovery.can_install ? (
                    <button
                      type="button"
                      onClick={() => void installJob(recovery.job.id, recovery.job)}
                      disabled={isBusy}
                      className="mt-4 h-10 rounded-[15px] bg-[#41f2ff] px-4 text-sm font-black text-[#111213] shadow-[0_0_10px_rgba(65,242,255,0.25)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-45"
                    >
                      继续安装
                    </button>
                  ) : null}
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-3 rounded-[18px] border border-dashed border-[var(--dm-border)] bg-black/25 px-4 py-6 text-sm font-semibold text-[var(--dm-muted-ink)]">
              当前没有可恢复的下载任务。
            </div>
          )}
        </div>

        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-black text-[var(--dm-muted-ink)]">安装历史</div>
              <div className="mt-1 text-[11px] font-bold text-slate-500">记录安装结果、完成时间，以及可回滚目录位置。</div>
            </div>
            <span className="rounded-full bg-[#41f2ff] px-3 py-1 text-[11px] font-black text-[#111213] shadow-[0_0_10px_rgba(65,242,255,0.3)]">{visibleInstallHistory.length}</span>
          </div>
          {visibleInstallHistory.length > 0 ? (
            <div className="mt-3 grid gap-3">
              {visibleInstallHistory.map((record) => (
                <div key={record.id} className="rounded-[18px] bg-black/30 border border-[#41f2ff]/10 p-4">
                  <div className="flex flex-wrap items-center gap-2 text-[11px] font-black text-[var(--dm-muted-ink)]">
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{record.version}</span>
                    <span className={`rounded-full px-3 py-1 border ${record.status === "completed" ? "bg-emerald-500/15 text-emerald-400 border-emerald-500/30" : "bg-red-500/15 text-red-400 border-red-500/30"}`}>
                      {installHistoryStatusLabel(record.status)}
                    </span>
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{record.package_kind}</span>
                  </div>
                  <div className="mt-3 text-xs font-bold text-[var(--dm-muted-ink)]">完成时间：{formatCompletedAt(record.completed_at)}</div>
                  <div className="mt-2 break-all text-xs font-semibold text-slate-500">
                    回滚目录：{record.rollback_path ?? "未记录"}
                  </div>
                  {record.error ? <div className="mt-2 text-xs font-bold text-red-400">{getUpdateErrorMessage(record.error)}</div> : null}
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-3 rounded-[18px] border border-dashed border-[var(--dm-border)] bg-black/25 px-4 py-6 text-sm font-semibold text-[var(--dm-muted-ink)]">
              当前还没有安装历史记录。
            </div>
          )}
        </div>
      </div>

      {visibleNotice ? <div className="mt-4 rounded-2xl border border-[#41f2ff]/10 bg-[#41f2ff]/5 px-4 py-3 text-sm font-semibold text-[#41f2ff]">{visibleNotice}</div> : null}
      {visibleError ? <div className="mt-4 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-red-400">{visibleError}</div> : null}
    </section>
  );
}
