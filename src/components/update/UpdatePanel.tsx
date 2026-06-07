import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { checkClientUpdate, getDefaultClient, installDownloadedUpdate, startUpdateDownload } from "../../lib/tauri";
import type { ClientInstallation, ClientUpdateCheck, DownloadJob, NetworkRouteConfig, NetworkRouteMode } from "../../types";

const MANIFEST_URL_PLACEHOLDER = "输入项目自维护 manifest 地址";

function formatAssetSize(size: number) {
  if (size >= 1024 * 1024) {
    return `${(size / (1024 * 1024)).toFixed(2)} MB`;
  }

  if (size >= 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${size} B`;
}

function progressPercent(job: DownloadJob | null) {
  if (!job || job.size === 0) {
    return 0;
  }

  return Math.min(100, Math.round((job.downloaded_bytes / job.size) * 100));
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

function errorText(error: unknown) {
  const raw = error instanceof Error ? error.message : String(error);
  if (raw.includes("host is not trusted") || raw.includes("host is not enabled")) {
    return "当前下载地址未被允许，请检查更新源或网络设置。";
  }
  if (raw.includes("must use https")) {
    return "请使用 HTTPS 地址。";
  }
  if (raw.includes("checksum") || raw.includes("sha256")) {
    return "下载文件校验失败，请重新下载。";
  }
  if (raw.includes("not found")) {
    return "没有找到对应的客户端或更新任务。";
  }
  if (raw.includes("running")) {
    return "请先关闭正在运行的客户端，再安装更新。";
  }
  if (raw.includes("manifest")) {
    return "更新源读取失败，请检查地址后重试。";
  }

  return "操作失败，请稍后重试。";
}

function networkRouteLabel(mode: NetworkRouteMode) {
  switch (mode) {
    case "direct":
      return "直接下载";
    case "proxy_prefix":
      return "代理前缀";
    case "mirror_template":
      return "镜像模板";
  }
}

function routeHostFromUrl(value: string) {
  try {
    const url = new URL(value);
    return url.hostname;
  } catch {
    return "";
  }
}

export function UpdatePanel() {
  const [manifestUrl, setManifestUrl] = useState("");
  const [channel, setChannel] = useState("stable");
  const [routeMode, setRouteMode] = useState<NetworkRouteMode>("direct");
  const [routeUrl, setRouteUrl] = useState("");
  const [client, setClient] = useState<ClientInstallation | null>(null);
  const [update, setUpdate] = useState<ClientUpdateCheck | null>(null);
  const [job, setJob] = useState<DownloadJob | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const latestRequestIdRef = useRef(0);

  useEffect(() => {
    let alive = true;
    void getDefaultClient()
      .then((nextClient) => {
        if (alive) {
          setClient(nextClient);
        }
      })
      .catch((err) => {
        if (alive) {
          setError(errorText(err));
        }
      });

    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let cleanup: UnlistenFn | undefined;
    let cleanupCompleted: UnlistenFn | undefined;
    let cleanupFailed: UnlistenFn | undefined;

    void listen<DownloadJob>("download-progress", (event) => setJob(event.payload)).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanup = fn;
    });
    void listen<DownloadJob>("download-completed", (event) => setJob(event.payload)).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupCompleted = fn;
    });
    void listen<DownloadJob>("download-failed", (event) => setJob(event.payload)).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanupFailed = fn;
    });

    return () => {
      disposed = true;
      cleanup?.();
      cleanupCompleted?.();
      cleanupFailed?.();
    };
  }, []);

  const resetResult = () => {
    latestRequestIdRef.current += 1;
    setUpdate(null);
    setJob(null);
    setError(null);
  };

  const buildNetworkRoute = (): NetworkRouteConfig | null => {
    if (routeMode === "direct") {
      return null;
    }

    const trimmedUrl = routeUrl.trim();
    const host = routeHostFromUrl(trimmedUrl);
    if (!trimmedUrl || !host) {
      throw new Error("route_url_invalid");
    }

    return {
      mode: routeMode,
      proxy_prefix_url: routeMode === "proxy_prefix" ? trimmedUrl : null,
      mirror_template: routeMode === "mirror_template" ? trimmedUrl : null,
      enabled_hosts: [host]
    };
  };

  const check = async () => {
    if (!client) {
      setError("请先在客户端管理中保存默认客户端。");
      return;
    }
    const trimmedManifestUrl = manifestUrl.trim();
    if (!trimmedManifestUrl) {
      setError("请先填写项目自维护的更新源地址。");
      return;
    }

    const requestId = latestRequestIdRef.current + 1;
    latestRequestIdRef.current = requestId;
    setError(null);
    setUpdate(null);
    setJob(null);
    setIsBusy(true);

    try {
      const networkRoute = buildNetworkRoute();
      const result = await checkClientUpdate({
        client_id: client.client_id,
        channel,
        manifest_url: trimmedManifestUrl,
        network_route: networkRoute
      });
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setUpdate(result);
      if (!result) {
        setError("更新源里没有适合当前客户端的版本。");
      }
    } catch (err) {
      if (latestRequestIdRef.current === requestId) {
        setError(err instanceof Error && err.message === "route_url_invalid" ? "请输入有效的网络地址。" : errorText(err));
      }
    } finally {
      if (latestRequestIdRef.current === requestId) {
        setIsBusy(false);
      }
    }
  };

  const download = async () => {
    if (!client || !update) {
      return;
    }
    const trimmedManifestUrl = manifestUrl.trim();
    if (!trimmedManifestUrl) {
      setError("请先填写项目自维护的更新源地址。");
      return;
    }

    setError(null);
    setIsBusy(true);
    try {
      const networkRoute = buildNetworkRoute();
      const nextJob = await startUpdateDownload({
        client_installation_id: client.id,
        channel: update.channel,
        manifest_url: trimmedManifestUrl,
        network_route: networkRoute
      });
      setJob(nextJob);
    } catch (err) {
      setError(err instanceof Error && err.message === "route_url_invalid" ? "请输入有效的网络地址。" : errorText(err));
    } finally {
      setIsBusy(false);
    }
  };

  const install = async () => {
    if (!job || job.status !== "verified") {
      return;
    }

    setError(null);
    setIsBusy(true);
    try {
      setJob(await installDownloadedUpdate(job.id));
    } catch (err) {
      setError(errorText(err));
    } finally {
      setIsBusy(false);
    }
  };

  const percent = progressPercent(job);

  return (
    <section className="rounded-[28px] border border-[var(--dm-border)] bg-white/82 p-6 text-[var(--dm-ink)] shadow-[0_24px_70px_rgba(47,52,64,0.10)] backdrop-blur-2xl">
      <h2 className="text-3xl font-black tracking-[-0.05em]">客户端更新</h2>
      <p className="mt-2 text-sm font-semibold text-[#4f5663]">检查默认客户端是否有新版本，下载完成后会先校验文件，再安装。</p>

      <div className="mt-5 grid gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <label className="block text-[11px] font-black tracking-[0.18em] text-[#59606d]" htmlFor="manifest-url-input">
            更新源地址
          </label>
          <input
            id="manifest-url-input"
            value={manifestUrl}
            onChange={(event) => {
              resetResult();
              setManifestUrl(event.target.value);
            }}
            disabled={isBusy}
            className="mt-3 h-12 w-full rounded-[16px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
            placeholder={MANIFEST_URL_PLACEHOLDER}
            spellCheck={false}
          />
        </div>

        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <label className="block text-[11px] font-black tracking-[0.18em] text-[#59606d]" htmlFor="channel-input">
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
            className="mt-3 h-12 w-full rounded-[16px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
            spellCheck={false}
          />
        </div>
      </div>

      <div className="mt-4 rounded-[22px] bg-[var(--dm-soft)] p-4">
        <div className="text-xs font-black text-[#59606d]">下载网络</div>
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
                  ? "border-[var(--dm-ink)] bg-[var(--dm-ink)] text-white"
                  : "border-[var(--dm-border)] bg-white text-[#59606d]"
              }`}
            >
              {networkRouteLabel(mode)}
            </button>
          ))}
        </div>
        {routeMode !== "direct" ? (
          <input
            value={routeUrl}
            onChange={(event) => {
              resetResult();
              setRouteUrl(event.target.value);
            }}
            disabled={isBusy}
            className="mt-3 h-12 w-full rounded-[16px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
            placeholder={routeMode === "proxy_prefix" ? "填写你的代理前缀地址" : "填写包含 {url} 的镜像模板"}
            spellCheck={false}
          />
        ) : null}
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[280px_minmax(0,1fr)]">
        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="text-xs font-black text-[#59606d]">默认客户端</div>
            <div className="mt-3 rounded-[18px] bg-white/76 p-4">
              <div className="text-lg font-black">{client?.display_name ?? "未设置"}</div>
              <div className="mt-2 break-all text-xs font-bold text-[#59606d]">{client?.install_dir ?? "请先保存默认客户端"}</div>
              <div className="mt-3 text-xs font-black text-[#3d4350]">
              {client ? `当前版本：${client.version ?? "未知"}` : "-"}
              </div>
          </div>
          <button
            onClick={() => void check()}
            disabled={!client || isBusy}
            className="mt-4 h-11 w-full rounded-[16px] bg-[var(--dm-ink)] px-5 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
          >
            {isBusy ? "请稍候" : "检查更新"}
          </button>
        </div>

        <div className="rounded-[22px] bg-[var(--dm-soft)] p-4">
          <div className="text-xs font-black text-[#59606d]">可用更新</div>
          {update ? (
            <div className="mt-3 rounded-[18px] bg-white/78 p-4">
              <div className="flex flex-wrap items-center gap-2 text-xs font-black text-[#59606d]">
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.channel}</span>
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.latest_version}</span>
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{update.asset.platform}</span>
                <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{formatAssetSize(update.asset.size)}</span>
              </div>
              <div className="mt-3 text-xs font-black text-[#59606d]">
                {update.needs_update ? "需要更新" : "当前已是最新版本"}
              </div>
              <button
                onClick={() => void download()}
                disabled={!update.needs_update || isBusy}
                className="mt-4 h-10 rounded-[15px] bg-[var(--dm-ink)] px-4 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
              >
                开始下载
              </button>
            </div>
          ) : (
            <div className="mt-3 rounded-[18px] border border-dashed border-[var(--dm-border)] bg-white/64 px-4 py-6 text-sm font-semibold text-[#59606d]">
              检查更新后会显示可下载的版本。
            </div>
          )}

          {job ? (
            <div className="mt-4 rounded-[18px] bg-white/78 p-4">
              <div className="flex items-center justify-between text-xs font-black text-[#59606d]">
                <span>{downloadStatusLabel(job.status)}</span>
                <span>{percent}%</span>
              </div>
              <div className="mt-3 h-2 overflow-hidden rounded-full bg-[var(--dm-soft)]">
                <div className="h-full bg-[var(--dm-ink)] transition-all" style={{ width: `${percent}%` }} />
              </div>
              <div className="mt-3 text-xs leading-6 text-[#59606d]">
                {job.status === "verified" ? "下载完成，文件已通过校验。" : "下载文件会保存在应用缓存中。"}
              </div>
              {job.error ? <div className="mt-2 text-xs font-bold text-[#8f2f2f]">{errorText(job.error)}</div> : null}
              <button
                onClick={() => void install()}
                disabled={job.status !== "verified" || isBusy}
                className="mt-4 h-10 rounded-[15px] border border-[var(--dm-ink)] px-4 text-sm font-black text-[var(--dm-ink)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-45"
              >
                安装更新
              </button>
            </div>
          ) : null}
        </div>
      </div>

      {error ? <div className="mt-4 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">{error}</div> : null}
    </section>
  );
}
