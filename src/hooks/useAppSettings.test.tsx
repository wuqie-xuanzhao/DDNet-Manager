import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../lib/settings";
import { useAppSettings } from "./useAppSettings";

const loadAppSettings = vi.fn();
const saveAppSettings = vi.fn();

vi.mock("../lib/tauri", () => ({
  loadAppSettings: (...args: unknown[]) => loadAppSettings(...args),
  saveAppSettings: (...args: unknown[]) => saveAppSettings(...args)
}));

describe("useAppSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadAppSettings.mockResolvedValue(defaultAppSettings);
    saveAppSettings.mockResolvedValue({
      ...defaultAppSettings,
      auto_check_updates: true
    });
  });

  it("keeps newer draft edits when an older save request finishes", async () => {
    const { result } = renderHook(() => useAppSettings(true));

    await waitFor(() => {
      expect(result.current.settingsState).toBe("idle");
    });

    await act(async () => {
      result.current.changeSettings({
        ...defaultAppSettings,
        auto_check_updates: true
      });
    });

    let resolveSave: (value: typeof defaultAppSettings) => void = () => undefined;
    saveAppSettings.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSave = resolve;
      })
    );

    void act(() => {
      void result.current.saveSettings();
    });

    await waitFor(() => {
      expect(result.current.settingsState).toBe("saving");
    });

    await act(async () => {
      result.current.changeSettings({
        ...defaultAppSettings,
        auto_check_updates: false,
        advanced_manifest_url: "https://example.com/manifest.json"
      });
    });

    await act(async () => {
      resolveSave({
        ...defaultAppSettings,
        auto_check_updates: true
      });
    });

    expect(result.current.savedAppSettings.auto_check_updates).toBe(true);
    expect(result.current.appSettings).toEqual({
      ...defaultAppSettings,
      auto_check_updates: false,
      advanced_manifest_url: "https://example.com/manifest.json"
    });
  });
});
