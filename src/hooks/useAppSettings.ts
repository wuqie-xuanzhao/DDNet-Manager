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

  const saveSettings = async () => {
    const appSettings = draftAppSettings ?? defaultAppSettings;
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
    saveSettings
  };
}
