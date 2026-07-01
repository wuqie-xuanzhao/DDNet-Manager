import { useEffect, useRef, useState } from "react";
import { getErrorMessage } from "../lib/errors";
import { defaultAppSettings } from "../lib/settings";
import { loadAppSettings, saveAppSettings } from "../lib/tauri";
import type { AppSettings } from "../types";

export type SettingsSaveState = "idle" | "loading" | "saving" | "saved" | "error";

export function useAppSettings(tauriRuntime: boolean) {
  const [loadedAppSettings, setLoadedAppSettings] = useState<AppSettings | null>(null);
  const [draftAppSettings, setDraftAppSettings] = useState<AppSettings | null>(null);
  const [runtimeSettingsState, setRuntimeSettingsState] = useState<Exclude<SettingsSaveState, "loading">>("idle");
  const [runtimeSettingsError, setRuntimeSettingsError] = useState<string | null>(null);
  const draftVersionRef = useRef(0);

  useEffect(() => {
    if (!tauriRuntime) {
      return;
    }

    let alive = true;

    void loadAppSettings()
      .then((settings) => {
        if (!alive) {
          return;
        }
        draftVersionRef.current += 1;
        setDraftAppSettings(settings);
        setLoadedAppSettings(settings);
        setRuntimeSettingsState("idle");
        setRuntimeSettingsError(null);
      })
      .catch((error) => {
        if (!alive) {
          return;
        }
        setRuntimeSettingsState("error");
        setRuntimeSettingsError(getErrorMessage(error));
      });

    return () => {
      alive = false;
    };
  }, [tauriRuntime]);

  const changeSettings = (settings: AppSettings) => {
    draftVersionRef.current += 1;
    setDraftAppSettings(settings);
    setRuntimeSettingsState("idle");
    setRuntimeSettingsError(null);
  };

  const saveSettings = async (explicitSettings?: AppSettings) => {
    const appSettings = explicitSettings ?? draftAppSettings ?? defaultAppSettings;
    const saveDraftVersion = draftVersionRef.current;
    setRuntimeSettingsState("saving");
    setRuntimeSettingsError(null);
    try {
      const savedSettings = await saveAppSettings(appSettings);
      setLoadedAppSettings(savedSettings);
      if (draftVersionRef.current === saveDraftVersion) {
        setDraftAppSettings(savedSettings);
      }
      setRuntimeSettingsState("saved");
    } catch (error) {
      setRuntimeSettingsState("error");
      setRuntimeSettingsError(getErrorMessage(error));
    }
  };

  /** Update settings draft and immediately persist to backend. */
  const updateAndSave = async (settings: AppSettings) => {
    draftVersionRef.current += 1;
    setDraftAppSettings(settings);
    await saveSettings(settings);
  };

  /**
   * 静默更新并保存设置：不修改 settingsState / settingsError，
   * 避免后台自动操作（如首次扫描标记）触发 UI 保存提示。
   */
  const updateAndSaveSilently = async (settings: AppSettings) => {
    draftVersionRef.current += 1;
    setDraftAppSettings(settings);
    try {
      const savedSettings = await saveAppSettings(settings);
      setLoadedAppSettings(savedSettings);
      setDraftAppSettings(savedSettings);
    } catch (error) {
      // 静默失败，仅控制台记录，不打扰用户
      console.error("Silent save failed:", error);
    }
  };

  const appSettings = tauriRuntime ? (draftAppSettings ?? defaultAppSettings) : defaultAppSettings;
  const savedAppSettings = tauriRuntime ? (loadedAppSettings ?? defaultAppSettings) : defaultAppSettings;
  const settingsState: SettingsSaveState = tauriRuntime ? runtimeSettingsState : "idle";
  const visibleSettingsState: SettingsSaveState =
    tauriRuntime && !loadedAppSettings && runtimeSettingsState === "idle" ? "loading" : settingsState;
  const settingsError = tauriRuntime ? runtimeSettingsError : null;

  return {
    appSettings,
    savedAppSettings,
    settingsState: visibleSettingsState,
    settingsError,
    changeSettings,
    saveSettings,
    updateAndSave,
    updateAndSaveSilently
  };
}
