import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  checkClientUpdate,
  createShortcuts,
  getClientCatalog,
  launchClient,
  listClientInstallations,
  removeClientInstallation,
  scanClientsViaMft,
  startUpdateDownload,
  upsertClientInstallation
} from "../lib/tauri";
import { buildStartUpdateDownloadRequest, buildUpdateSourceRequest } from "../lib/updateLogic";
import { appendScanEventCapped, type ScanProgressEvent } from "../lib/scanProgress";
import { getUpdateErrorMessage } from "../lib/errors";
import type {
  AppSettings,
  ClientCatalogEntry,
  ClientInstallation,
  ClientUpdateCheck,
  DownloadJob
} from "../types";

/// 单个客户端的安装状态机。驱动 DownloadButton 主按钮 + 小链接的视觉与行为。
export type ClientInstallState =
  | { kind: "loading" }                                                                  // 初始化中
  | { kind: "unknown" }                                                                  // registry 无记录、未扫过
  | { kind: "not_installed"; scanned: boolean }                                          // 扫描完成确认没装
  | {
      kind: "installed";
      version: string | null;
      latest: string | null;
      needsUpdate: boolean;
      assetSize: number | null;
      releaseUrl: string | null;
    }
  | { kind: "broken"; reason: string }                                                   // 已装但 health != Ok
  | { kind: "downloading"; progress: number; speedBytesPerSec: number; paused: boolean }
  | { kind: "verifying"; progress: number }
  | { kind: "failed"; error: string };

export type InstallDialogMode = "install" | "update";

/// verify-progress 事件 payload（Rust 端 TauriVerifySink emit，#[serde(rename_all =
/// "camelCase")] 让 client_installation_id → clientInstallationId）。
/// 前端 useClientInstaller 按 clientInstallationId 判断事件归属。
interface VerifyProgressPayload {
  clientInstallationId: string;
  bytesRead: number;
  total: number;
}

const RELEASE_CACHE_TTL_MS = 5 * 60 * 1000;

/// 跨 tab 共享的 release 元数据缓存：client_id → { check, fetchedAt }。
/// 避免每个 tab 都打一次 GitHub API。组件层用模块级 Map 持久化。
type ReleaseCacheEntry = {
  check: ClientUpdateCheck | null;
  fetchedAt: number;
};
const releaseCache = new Map<string, ReleaseCacheEntry>();

/// 跨 tab 共享的 catalog 缓存：避免每个 useClientInstaller 实例都拉一次。
let catalogCache: ClientCatalogEntry[] | null = null;
let catalogFetchPromise: Promise<ClientCatalogEntry[]> | null = null;

// review issue #12：dev HMR 下模块级缓存不清空会导致状态错乱。
// 监听 Vite HMR dispose 钩子，模块重新加载时清空缓存。
if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    releaseCache.clear();
    catalogCache = null;
    catalogFetchPromise = null;
  });
}

async function fetchCatalog(): Promise<ClientCatalogEntry[]> {
  if (catalogCache) return catalogCache;
  if (catalogFetchPromise) return catalogFetchPromise;
  catalogFetchPromise = getClientCatalog()
    .then((c) => {
      catalogCache = c;
      return c;
    })
    .finally(() => {
      catalogFetchPromise = null;
    });
  return catalogFetchPromise;
}

/// 把 launcher 的 gameId（"ddnet" / "qmclient" / "taterclient" / "bestclient" / "cactusclient"）
/// 映射到 registry 客户端是否匹配。
/// - ddnet：同时识别 Steam 安装和官网下载版（合并到一个 tab）
/// - 其他：精确匹配 client_id
function clientMatchesGameId(client: ClientInstallation, gameId: string): boolean {
  if (gameId === "ddnet") {
    return client.client_id === "ddnet" || client.client_id === "ddnet_vanilla";
  }
  if (gameId === "qmclient") {
    return client.client_id === "qmclient" || client.client_id === "qmclient_nightly";
  }
  return client.client_id === gameId;
}

/// 把 launcher 的 gameId 映射到 catalog 的 client_id（用于拉 release）。
/// 当前所有 launcher gameId 都直接对应 catalog client_id。
function gameIdToCatalogClientId(gameId: string): string | null {
  return gameId;
}

export function useClientInstaller(params: {
  gameId: string;
  appSettings: AppSettings;
  tauriRuntime: boolean;
}) {
  const { gameId, appSettings, tauriRuntime } = params;
  const [state, setState] = useState<ClientInstallState>({ kind: "loading" });
  const [client, setClient] = useState<ClientInstallation | null>(null);
  /// 同 gameId 匹配的所有客户端副本（用户装了多个 QmClient 时 > 1）。
  /// selectedClient 默认取 is_default 或第一个，用户可通过 selectClient() 切换。
  const [clients, setClients] = useState<ClientInstallation[]>([]);
  /// 当前 gameId 对应的 catalog entry。InstallDialog 用它判断更新源类型
  /// （github_release / ddnet_official 显示版本卡片；website/none 显示"打开官网下载"）。
  const [catalogEntry, setCatalogEntry] = useState<ClientCatalogEntry | null>(null);
  const [installDialogOpen, setInstallDialogOpen] = useState(false);
  const [installDialogMode, setInstallDialogMode] = useState<InstallDialogMode>("install");
  const [scanning, setScanning] = useState(false);
  /// 最近一次扫描的事件流（cap 50，B3 防止长时间扫描后内存爆）。
  /// triggerScan 开始时清空，scan-progress 事件到来时累积。InstallDialog
  /// 在 scanning 时渲染最近几条 describeScanEvent 给用户实时反馈。
  const [scanEvents, setScanEvents] = useState<ScanProgressEvent[]>([]);

  // 当前 tab 的 requestId：切 tab 时让旧 tab 的异步操作作废。
  const requestIdRef = useRef(0);
  const releaseFetchingRef = useRef(false);

  /// 进入 failed state 的统一入口。C3：把原始 err/message 经 getUpdateErrorMessage
  /// 映射成中文友好消息（按 IPC 错误码 + 关键字兜底），并可选弹 sonner toast。
  /// 下载/安装失败这种用户主动操作触发弹 toast；后台 refresh 失败等静默场景不弹。
  const setFailedState = useCallback(
    (error: unknown, options: { toast: boolean } = { toast: false }) => {
      const friendly = getUpdateErrorMessage(error);
      setState({ kind: "failed", error: friendly });
      if (options.toast) {
        toast.error(friendly);
      }
    },
    []
  );

  /// 从 registry 拉匹配 gameId 的客户端，推导 state。
  /// 同步触发 release 元数据刷新（缓存过期时）。
  const refreshFromRegistry = useCallback(async () => {
    if (!tauriRuntime) {
      setState({ kind: "unknown" });
      return;
    }
    const requestId = ++requestIdRef.current;
    setState({ kind: "loading" });

    try {
      const installations = await listClientInstallations();
      if (requestId !== requestIdRef.current) return;
      const allMatched = installations.filter((inst) => clientMatchesGameId(inst, gameId));
      // 默认选 is_default，否则第一个
      const matched = allMatched.find((c) => c.is_default) ?? allMatched[0] ?? null;
      setClients(allMatched);
      setClient(matched);

      if (!matched) {
        setState({ kind: "unknown" });
        return;
      }
      if (matched.health !== "ok") {
        setState({ kind: "broken", reason: matched.missing_items.join("、") || "客户端不完整" });
        return;
      }
      // installed：等 release 元数据回来再决定 needsUpdate
      setState({
        kind: "installed",
        version: matched.version ?? null,
        latest: null,
        needsUpdate: false,
        assetSize: null,
        releaseUrl: null
      });
    } catch (err) {
      if (requestId !== requestIdRef.current) return;
      // refresh 失败静默：后台刷新不弹 toast 干扰用户
      setFailedState(err);
    }
  }, [gameId, setFailedState, tauriRuntime]);

  /// 把 release 元数据合并到当前 installed state。
  /// 只在 state 是 installed 时刷新 latest/needsUpdate/assetSize/releaseUrl。
  /// 必须声明在 fetchRelease 前（fetchRelease 的缓存命中分支会调它）。
  const applyReleaseToState = useCallback((check: ClientUpdateCheck | null) => {
    setState((prev) => {
      if (prev.kind !== "installed") return prev;
      return {
        ...prev,
        latest: check?.latest_version ?? null,
        needsUpdate: Boolean(check?.needs_update),
        assetSize: check?.asset.size ?? null,
        releaseUrl: check?.action_url ?? null
      };
    });
  }, []);

  /// 拉当前 tab 对应客户端的 GitHub Release 元数据。命中 5min 缓存直接返回。
  /// release 拉到后，如果当前是 installed 状态，刷新 latest / needsUpdate / assetSize / releaseUrl。
  const fetchRelease = useCallback(async () => {
    if (!tauriRuntime) return;
    const catalogClientId = gameIdToCatalogClientId(gameId);
    if (!catalogClientId) return;

    // 缓存命中
    const cached = releaseCache.get(catalogClientId);
    if (cached && Date.now() - cached.fetchedAt < RELEASE_CACHE_TTL_MS) {
      applyReleaseToState(cached.check);
      return;
    }
    if (releaseFetchingRef.current) return;
    releaseFetchingRef.current = true;

    const requestId = requestIdRef.current;
    try {
      const catalog = await fetchCatalog();
      const entry = catalog.find((e) => e.client_id === catalogClientId);
      if (entry) {
        setCatalogEntry(entry);
      }
      if (!entry) {
        releaseFetchingRef.current = false;
        return;
      }
      // 只对有 github_release 或 ddnet_official 来源的客户端拉 release。
      // website / none 不拉，前端只用 upstream_url 引导。
      if (entry.update_source.kind !== "github_release" && entry.update_source.kind !== "ddnet_official") {
        releaseFetchingRef.current = false;
        return;
      }

      const check = await checkClientUpdate(
        buildUpdateSourceRequest({
          clientId: catalogClientId,
          channel: "stable",
          manifestUrl: "",
          routeMode: appSettings.network_route?.mode ?? "direct",
          routeUrl: appSettings.network_route?.local_proxy_url ?? "",
          useManifestSource: false
        })
      );

      if (requestId !== requestIdRef.current) {
        releaseFetchingRef.current = false;
        return;
      }

      releaseCache.set(catalogClientId, { check, fetchedAt: Date.now() });
      applyReleaseToState(check);
    } catch (err) {
      console.warn(`[useClientInstaller] fetchRelease failed for ${catalogClientId}:`, err);
      // release 拉失败不阻断主流程，state 保持 installed（latest 为 null）
    } finally {
      releaseFetchingRef.current = false;
    }
  }, [appSettings.network_route, gameId, tauriRuntime, applyReleaseToState]);

  /// 启动时：先 refreshFromRegistry，再 fetchRelease。
  useEffect(() => {
    void refreshFromRegistry().then(() => void fetchRelease());
  }, [refreshFromRegistry, fetchRelease]);

  /// 监听 download-progress / download-completed / download-failed / install-* 事件。
  /// **关键设计**：监听器只在 mount 时挂一次（依赖 [tauriRuntime]），不依赖 client。
  /// 通过 clientRef 让 handler 始终拿到最新 client.id 做事件归属判断（isMine）。
  /// 这样 beginInstall 切 client 后不需要重建监听器，避免 download 事件在
  /// "旧 unlisten 完成 + 新 listen 还没 resolve" 间隙丢失（review issue #1）。
  const clientRef = useRef<ClientInstallation | null>(client);
  useEffect(() => {
    clientRef.current = client;
  }, [client]);

  useEffect(() => {
    if (!tauriRuntime) return;

    // review issue #4：用 Promise.all 并行 listen 所有事件，单个 await 点。
    // dispose 在 await 之前触发时所有 unlisten 一次性调用；
    // dispose 在 await 之后触发时 unlistens 已稳定填充，cleanup 调用所有。
    // 避免 for 循环里逐个 await + push 顺序乱导致的 cleanup race。
    let unlistens: UnlistenFn[] = [];
    let disposed = false;

    const setup = async () => {
      const isMine = (job: DownloadJob) => {
        const current = clientRef.current;
        return current !== null && job.client_installation_id === current.id;
      };
      const isVerifyMine = (payload: { clientInstallationId: string }) => {
        const current = clientRef.current;
        return current !== null && payload.clientInstallationId === current.id;
      };

      const handlers: Array<[string, (e: { payload: DownloadJob | string | VerifyProgressPayload }) => void]> = [
        [
          "download-progress",
          (e) => {
            const job = e.payload as DownloadJob;
            if (!isMine(job)) return;
            setState({
              kind: "downloading",
              progress: job.downloaded_bytes / Math.max(1, job.size),
              speedBytesPerSec: 0,
              paused: false
            });
          }
        ],
        [
          "download-completed",
          (e) => {
            const job = e.payload as DownloadJob;
            if (!isMine(job)) return;
            // 进入 verify 阶段：progress 起始为 0，verify-progress 事件会持续刷新
            setState({ kind: "verifying", progress: 0 });
          }
        ],
        [
          "verify-progress",
          (e) => {
            const payload = e.payload as VerifyProgressPayload;
            if (!isVerifyMine(payload)) return;
            setState({
              kind: "verifying",
              progress: payload.total > 0 ? payload.bytesRead / payload.total : 0
            });
          }
        ],
        [
          "download-failed",
          (e) => {
            const job = e.payload as DownloadJob;
            if (!isMine(job)) return;
            // 下载失败弹 toast：用户主动操作触发，需要明确反馈
            setFailedState(job.error ?? "下载失败", { toast: true });
          }
        ],
        [
          "install-completed",
          async (e) => {
            const job = e.payload as DownloadJob;
            if (!isMine(job)) return;
            const shortcutOpts = pendingShortcutOptionsRef.current;
            const currentClient = clientRef.current;
            if (shortcutOpts && currentClient) {
              try {
                await createShortcuts({
                  executable_path: currentClient.executable_path,
                  working_dir: currentClient.install_dir,
                  display_name: currentClient.display_name,
                  desktop: shortcutOpts.desktop,
                  start_menu: shortcutOpts.startMenu
                });
              } catch (err) {
                // C4: 快捷方式失败不再被吞——客户端已成功安装但快捷方式没创建，
                // 用户不知道为什么桌面/开始菜单没图标。toast 显式提示+保留日志便于排查。
                console.warn(`[useClientInstaller] create_shortcuts failed:`, err);
                toast.error(`快捷方式创建失败：${getUpdateErrorMessage(err)}`);
              }
              pendingShortcutOptionsRef.current = null;
            }
            // review issue L3：refreshFromRegistry 抛错不应导致 listen 回调内
            // promise rejection 未捕获。C3 已有 setFailedState，包一下统一入口。
            try {
              await refreshFromRegistry();
            } catch (err) {
              setFailedState(err, { toast: true });
            }
            void fetchRelease();
          }
        ],
        [
          "install-failed",
          (e) => {
            const job = e.payload as DownloadJob;
            if (!isMine(job)) return;
            // 安装失败弹 toast：用户主动操作触发
            setFailedState(job.error ?? "安装失败", { toast: true });
          }
        ]
      ];

      // 并行注册所有事件监听器，单次 await
      const results = await Promise.all(
        handlers.map(([event, handler]) => listen(event, handler))
      );

      if (disposed) {
        // cleanup 在 await 期间触发，立即调用所有 unlisten
        for (const u of results) u();
        return;
      }
      unlistens = results;
    };

    void setup();

    return () => {
      disposed = true;
      for (const u of unlistens) u();
      unlistens = [];
    };
  }, [tauriRuntime, refreshFromRegistry, fetchRelease, setFailedState]);

  /// 点"定位游戏"触发后台扫描。命中后 setClient + refreshFromRegistry 让 state
  /// 自动切到 installed；如果是从安装弹窗里点的（installDialogOpen=true），命中后
  /// 自动关闭弹窗（review issue #6）。
  ///
  /// B4 之后 Rust 端在 priority 命中时已自动 upsert 落 registry，前端不再二次调用
  /// upsert（fallback 命中或 health 非 ok 的 priority 命中除外，那两种情况
  /// Rust 端不落库，但 refreshFromRegistry 会从 registry 拉已存在的记录，所以
  /// UI 状态仍正确）。
  const triggerScan = useCallback(async () => {
    if (!tauriRuntime || scanning) return;
    setScanning(true);
    setScanEvents([]);
    try {
      const results = await scanClientsViaMft({
        include_saved_paths: true,
        include_unhealthy: false
      });
      const matched = results.find((inst: ClientInstallation) => clientMatchesGameId(inst, gameId)) ?? null;
      if (matched) {
        setClient(matched);
        await refreshFromRegistry();
        // 弹窗里点"定位游戏"命中后自动关闭弹窗
        if (installDialogOpen) {
          setInstallDialogOpen(false);
        }
      } else {
        setState({ kind: "not_installed", scanned: true });
      }
    } catch (err) {
      // triggerScan 失败：用户在 InstallDialog 点"定位游戏"，反馈走弹窗内的
      // failed 文案 + 进度区域，不弹 toast（避免遮挡弹窗）
      setFailedState(err);
    } finally {
      setScanning(false);
    }
  }, [gameId, installDialogOpen, refreshFromRegistry, scanning, setFailedState, tauriRuntime]);

  /// 点"获取游戏"主按钮：立即打开安装对话框（不阻塞）。
  /// 弹窗里的"已安装？定位游戏"小链接负责主动扫描，不在 openInstallDialog 里做。
  /// 这样用户点按钮瞬间看到弹窗，扫描反馈通过弹窗 UI 完成。
  const openInstallDialog = useCallback(() => {
    if (!tauriRuntime) return;
    setInstallDialogMode("install");
    setInstallDialogOpen(true);
  }, [tauriRuntime]);

  /// 点"获取更新 vX.Y.Z"小链接：已安装版本有更新，打开更新对话框（预填当前路径）。
  const openUpdateDialog = useCallback(() => {
    if (!tauriRuntime || !client) return;
    setInstallDialogMode("update");
    setInstallDialogOpen(true);
  }, [client, tauriRuntime]);

  /// 弹窗"开始安装"按钮：调 startUpdateDownload 启动真实下载。
  /// 接收弹窗收集的参数（targetClient 由调用方确定，installDir 现在由 upsert 时确定，
  /// 后续阶段 2.3/2.4 完善 path override / shortcut options）。
  const startDownloadFor = useCallback(
    async (target: ClientInstallation) => {
      try {
        setState({ kind: "downloading", progress: 0, speedBytesPerSec: 0, paused: false });
        await startUpdateDownload(
          buildStartUpdateDownloadRequest({
            clientInstallationId: target.id,
            channel: "stable",
            manifestUrl: "",
            routeMode: appSettings.network_route?.mode ?? "direct",
            routeUrl: appSettings.network_route?.local_proxy_url ?? "",
            useManifestSource: false
          })
        );
        // 实际进度通过 download-progress 事件流回，state 会被 listener 刷新
      } catch (err) {
        // startUpdateDownload 调用失败：用户主动操作触发，弹 toast 反馈
        setFailedState(err, { toast: true });
      }
    },
    [appSettings.network_route, setFailedState]
  );

  /// 弹窗"开始安装"按钮的真实入口：接收弹窗收集的 installDir + shortcut 选项，
  /// 先 upsert client 落 registry（用新 installDir），再 startDownloadFor。
  /// shortcut 选项传递给后续 install-completed 事件触发的 create_shortcuts。
  const pendingShortcutOptionsRef = useRef<{ desktop: boolean; startMenu: boolean } | null>(null);

  const beginInstall = useCallback(
    async (params: { installDir: string; desktop: boolean; startMenu: boolean }) => {
      if (!tauriRuntime) return;
      // install 模式（新建路径）：失败时回滚 upsert，避免下次 refresh 显示 installed
      // 与 failed 文案矛盾（review issue #2）。update 模式不回滚（保留原记录）。
      const shouldRollbackOnFailure = installDialogMode === "install";
      let upsertedId: string | null = null;
      try {
        const upserted = await upsertClientInstallation({
          install_dir: params.installDir,
          is_default: false
        });
        upsertedId = upserted.id;
        setClient(upserted);
        // pendingShortcutOptionsRef 在事件回调（非 effect）里写：beginInstall 是用户
        // 触发的安装动作，install-completed 事件触发时读它决定是否调 create_shortcuts。
        // react-hooks/immutability 规则把"ref 在 effect 里被读"判定为不可在外部写，
        // 这里是用户事件触发，不在 effect 链上，eslint-disable 显式豁免。
        // eslint-disable-next-line react-hooks/immutability
        pendingShortcutOptionsRef.current = { desktop: params.desktop, startMenu: params.startMenu };
        setInstallDialogOpen(false);
        await startDownloadFor(upserted);
      } catch (err) {
        // startDownloadFor 内部已 setState failed。这里负责回滚 registry：
        // install 模式下移除刚创建的占位记录，下次 refresh 回到 unknown 状态。
        if (shouldRollbackOnFailure && upsertedId) {
          try {
            await removeClientInstallation(upsertedId);
            setClient(null);
            setClients((prev) => prev.filter((c) => c.id !== upsertedId));
          } catch (rollbackErr) {
            console.warn(`[useClientInstaller] rollback failed:`, rollbackErr);
          }
        }
        // beginInstall 失败：用户主动操作触发，弹 toast 反馈
        setFailedState(err, { toast: true });
      }
    },
    [installDialogMode, setFailedState, startDownloadFor, tauriRuntime]
  );

  const closeInstallDialog = useCallback(() => {
    setInstallDialogOpen(false);
  }, []);

  /// 点"开始游戏"主按钮。真调 launchClient，启动后最小化启动器到托盘。
  /// 用 hide() 而非 minimize()：transparent + decorations:false 窗口的 minimize 在
  /// Windows 上行为不稳定（可能被 DWM 销毁触发意外事件）。hide() 是显式隐藏窗口对象，
  /// 窗口仍存在，托盘点击 show() 能稳定恢复。
  const launchGame = useCallback(async () => {
    if (!tauriRuntime || !client) return;
    try {
      await launchClient(client.executable_path);
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().hide();
      } catch (hideErr) {
        console.warn(`[useClientInstaller] hide launcher after launch failed:`, hideErr);
      }
    } catch (err) {
      // launchGame 失败：用户主动操作触发，弹 toast 反馈
      setFailedState(err, { toast: true });
    }
  }, [client, setFailedState, tauriRuntime]);

  /// 切换 selectedClient 到指定副本（用户装了多个同 gameId 客户端时用）。
  /// 切换后 state 自动按新 client.health / version 重新评估。
  const selectClient = useCallback(
    (id: string) => {
      const target = clients.find((c) => c.id === id);
      if (!target || target.id === client?.id) return;
      setClient(target);
      if (target.health !== "ok") {
        setState({ kind: "broken", reason: target.missing_items.join("、") || "客户端不完整" });
      } else {
        setState({
          kind: "installed",
          version: target.version ?? null,
          latest: null,
          needsUpdate: false,
          assetSize: null,
          releaseUrl: null
        });
        // 切换后重新拉 release 判断是否需要更新
        void fetchRelease();
      }
    },
    [client, clients, fetchRelease]
  );

  /// 监听 scan-progress 事件，累积到 scanEvents 给 InstallDialog 渲染时间线。
  /// 只在 scanning 时挂监听，避免平时占用 IPC 通道。事件 cap 50（B3 策略）。
  useEffect(() => {
    if (!tauriRuntime || !scanning) return;
    let unlisten: UnlistenFn | null = null;
    let disposed = false;
    void listen<ScanProgressEvent>("scan-progress", (e) => {
      setScanEvents((prev) => appendScanEventCapped(prev, e.payload));
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [scanning, tauriRuntime]);

  /// 把 state 翻译成 DownloadButton 用的 4 态机（兼容现有组件 props）。
  /// 阶段 1.5 切换到 DownloadButton 时使用。
  const buttonProps = useMemo(() => {
    const isInstalled = state.kind === "installed";
    const isDownloading = state.kind === "downloading" || state.kind === "verifying";
    const canLaunch = isInstalled && client?.health === "ok";

    return {
      canLaunch: Boolean(canLaunch),
      disabled: state.kind === "loading" || state.kind === "failed",
      hasUpdate: isInstalled && state.kind === "installed" && state.needsUpdate,
      latestVersion: isInstalled && state.kind === "installed" ? state.latest : null,
      downloading: isDownloading,
      progress: state.kind === "downloading" ? state.progress : 0,
      broken: state.kind === "broken"
    };
  }, [state, client]);

  return {
    state,
    client,
    clients,
    catalogEntry,
    scanning,
    scanEvents,
    installDialogOpen,
    installDialogMode,
    buttonProps,
    triggerScan,
    openInstallDialog,
    openUpdateDialog,
    closeInstallDialog,
    beginInstall,
    launchGame,
    selectClient,
    refreshFromRegistry,
    fetchRelease
  };
}
