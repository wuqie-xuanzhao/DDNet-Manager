import { useCallback, useEffect, useRef, useState } from "react";
import { checkAppUpdate } from "../lib/tauri";
import { getErrorMessage } from "../lib/errors";
import type { AppSettings, AppUpdateCheck } from "../types";

/// 启动器自身更新检查状态。镜像 SettingsDialog 关于页"检查最新版本"的状态机，
/// 但用于右上角常驻按钮 + 启动后自动检查。
export type AppUpdaterState =
  | "idle"           // 未检查（启动器刚启动 / allow_silent_update=false）
  | "checking"       // 检查中
  | "up-to-date"     // 已是最新
  | "has-update"     // 发现新版本
  | "failed";        // 检查失败

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
  const lastCheckRef = useRef<number>(0);
  const inflightRef = useRef(false);
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

  return {
    state,
    updateInfo,
    error,
    checkForUpdate,
    /** 是否应该在右上角显示按钮：检查过（任何状态）或正在检查都显示；idle 不显示 */
    visible: state !== "idle" || appSettings.allow_silent_update
  };
}
