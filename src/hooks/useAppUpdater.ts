import { useCallback, useEffect, useRef, useState } from "react";
import { check as checkUpdater, type Update, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { checkAppUpdate } from "../lib/tauri";
import { getErrorMessage } from "../lib/errors";
import type { AppSettings, AppUpdateCheck } from "../types";

/// 启动器自身更新检查与安装状态机。
/// 合并 SettingsDialog 关于页"检查最新版本" + 右上角 UpdateControlButton 的完整生命周期：
/// idle → checking → (up-to-date | has-update | failed) → (downloading → installing → ready-to-restart) | failed
export type AppUpdaterState =
  | "idle"           // 未检查（启动器刚启动 / allow_silent_update=false）
  | "checking"       // 检查中
  | "up-to-date"     // 已是最新
  | "has-update"     // 发现新版本，等待用户点"立即更新"
  | "downloading"    // downloadAndInstall 进行中，progress 持续更新
  | "installing"     // 下载完成，installer 启动中（NSIS/MSI 执行阶段）
  | "ready-to-restart" // installer 结束，等待 relaunch
  | "failed";        // 检查或安装失败（看 error / installError 区分来源）

/// 下载进度快照。downloading 状态下持续更新。
export type AppUpdaterProgress = {
  /// 已下载字节数。
  downloaded: number;
  /// 总字节数（部分更新源可能不提供，此时为 0）。
  total: number;
  /// 0~1 之间的小数，total 未知时为 0。
  ratio: number;
};

const AUTO_CHECK_DELAY_MS = 1500; // 启动后延迟 1.5s 检查，避免和首屏渲染抢资源
const RECHECK_COOLDOWN_MS = 5 * 60 * 1000; // 手动重检查冷却 5min，防止用户狂点

export function useAppUpdater(params: {
  tauriRuntime: boolean;
  appSettings: AppSettings;
}) {
  const { tauriRuntime, appSettings } = params;
  const [state, setState] = useState<AppUpdaterState>("idle");
  const [updateInfo, setUpdateInfo] = useState<AppUpdateCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [progress, setProgress] = useState<AppUpdaterProgress | null>(null);
  const lastCheckRef = useRef<number>(0);
  const inflightRef = useRef(false);
  /// 当前正在进行的 Update 实例。downloading 状态下用于支持 cancel。
  /// 用 ref 持有不触发 re-render；cancel 时调用其内部 cancel channel。
  const activeUpdateRef = useRef<Update | null>(null);
  // review issue #11：用 ref 持有 updateInfo 让 checkForUpdate callback 依赖 [] 稳定。
  // 避免 updateInfo 变化时 callback 重建，传给 onClick 时每次渲染创建新函数。
  const updateInfoRef = useRef<AppUpdateCheck | null>(null);
  useEffect(() => {
    updateInfoRef.current = updateInfo;
  }, [updateInfo]);

  const checkForUpdate = useCallback(
    async (options?: { force?: boolean }) => {
      if (!tauriRuntime) return;
      if (inflightRef.current) return;
      const now = Date.now();
      if (!options?.force && now - lastCheckRef.current < RECHECK_COOLDOWN_MS && updateInfoRef.current) {
        return;
      }

      inflightRef.current = true;
      lastCheckRef.current = now;
      setState("checking");
      setError(null);

      try {
        const result = await checkAppUpdate();
        setUpdateInfo(result);
        setState(result.has_update ? "has-update" : "up-to-date");
      } catch (err) {
        setError(getErrorMessage(err));
        setState("failed");
      } finally {
        inflightRef.current = false;
      }
    },
    [tauriRuntime]
  );

  /// 用户点"立即更新"后触发：通过 tauri-plugin-updater 拉取签名校验过的安装包并执行。
  /// 进度通过 setProgress 流式更新；完成切到 ready-to-restart；失败切到 failed。
  /// 失败时 installError 持有错误信息（与 check 阶段的 error 分开）。
  const downloadAndInstall = useCallback(async () => {
    if (!tauriRuntime) return;
    if (activeUpdateRef.current) return; // 防止重复触发

    setInstallError(null);
    setProgress({ downloaded: 0, total: 0, ratio: 0 });
    setState("downloading");

    // 捕获本地引用。cancel/unmount 时 activeUpdateRef 会被置 null，
    // 后续 callback / await 后通过 myUpdate !== activeUpdateRef.current 检测"已放弃"，
    // 不再覆盖 UI 状态（plugin 2.x 没暴露真实 cancel，下载会继续到完成，但 UI 不再受影响）。
    let myUpdate: Update | null = null;

    try {
      const update = await checkUpdater();
      if (!update) {
        // 罕见情况：tauri-plugin-updater 认为已是最新（与 check_app_update 结论不一致）
        setState("up-to-date");
        setProgress(null);
        return;
      }
      activeUpdateRef.current = update;
      myUpdate = update;

      await update.downloadAndInstall((event: DownloadEvent) => {
        // 已被 cancel 或组件已卸载：丢弃后续 progress 回调
        if (activeUpdateRef.current !== myUpdate) return;
        switch (event.event) {
          case "Started":
            setProgress({
              downloaded: 0,
              total: Number(event.data.contentLength) || 0,
              ratio: 0,
            });
            break;
          case "Progress":
            setProgress((prev) => {
              const total = prev?.total ?? 0;
              const chunk = Number(event.data.chunkLength) || 0;
              const downloaded = (prev?.downloaded ?? 0) + chunk;
              return {
                downloaded,
                total,
                ratio: total > 0 ? Math.min(1, downloaded / total) : 0,
              };
            });
            break;
          case "Finished":
            // 下载完成，installer 即将启动；切到 installing 等待 downloadAndInstall Promise resolve
            setState("installing");
            break;
        }
      });

      // 已被 cancel：不覆盖 UI（用户已经回到 has-update），后台下载继续无害
      if (activeUpdateRef.current !== myUpdate) return;
      // downloadAndInstall Promise resolve = installer 执行完毕，等待重启
      activeUpdateRef.current = null;
      setProgress(null);
      setState("ready-to-restart");
    } catch (err) {
      // cancel 不会触发 plugin 抛错（下载继续），但如果 ref 已被置 null（unmount/cancel），
      // 直接吞掉错误避免把"卸载中的瞬时错误"误报为 install 失败
      if (myUpdate && activeUpdateRef.current !== myUpdate) return;
      activeUpdateRef.current = null;
      setInstallError(getErrorMessage(err));
      setProgress(null);
      setState("failed");
    }
  }, [tauriRuntime]);

  /// 取消正在进行的下载。tauri-plugin-updater 2.x 没暴露 cancel API，
  /// 通过丢弃 activeUpdateRef 让进行中的 Promise 不再影响 state（下载会继续在后台，
  /// 但 UI 立即切回 has-update）。后续版本若 plugin 加 cancel channel 再实装真实取消。
  const cancelDownload = useCallback(() => {
    activeUpdateRef.current = null;
    setProgress(null);
    setState(updateInfoRef.current?.has_update ? "has-update" : "idle");
  }, []);

  /// 重启启动器。relaunch 来自 tauri-plugin-process，调用后当前进程退出，新进程拉起。
  const restartNow = useCallback(async () => {
    if (!tauriRuntime) return;
    try {
      await relaunch();
    } catch (err) {
      setInstallError(getErrorMessage(err));
      setState("failed");
    }
  }, [tauriRuntime]);

  // 启动后自动检查：仅在 allow_silent_update=true 时触发。
  // 延迟 1.5s 避免和首屏 catalog/release 拉取抢网络资源。
  useEffect(() => {
    if (!tauriRuntime || !appSettings.allow_silent_update) {
      setState("idle");
      return;
    }
    const timer = setTimeout(() => {
      void checkForUpdate();
    }, AUTO_CHECK_DELAY_MS);
    return () => clearTimeout(timer);
    // 只在启动时跑一次；用户切换 allow_silent_update 不自动重检查（避免抖动）
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tauriRuntime]);

  // 卸载时丢弃 in-flight update 引用：进行中的 downloadAndInstall Promise 回调
  // 通过 myUpdate !== activeUpdateRef.current 守卫跳过 setState，避免操作已卸载组件。
  useEffect(() => {
    return () => {
      activeUpdateRef.current = null;
      inflightRef.current = false;
    };
  }, []);

  return {
    state,
    updateInfo,
    error,
    installError,
    progress,
    checkForUpdate,
    downloadAndInstall,
    cancelDownload,
    restartNow,
    /** 是否应该在右上角显示按钮：检查过（任何状态）或正在检查都显示；idle 不显示 */
    visible: state !== "idle" || appSettings.allow_silent_update
  };
}
