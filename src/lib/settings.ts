import type { AppSettings, NetworkRouteMode } from "../types";

export const defaultAppSettings: AppSettings = {
  network_route: null,
  scan_excluded_paths: [],
  scan_max_results: null,
  scan_timeout_secs: null,
  close_panel_after_launch: true,
  auto_check_updates: false,
  autostart: false,
  exit_game_show_launcher: true,
  close_behavior: "ask",
  allow_silent_update: true,
  extra_trusted_hosts: [],
  mirror_prefixes: [],
  has_scanned_clients: false
};

export function networkRouteUrl(settings: AppSettings) {
  return settings.network_route?.local_proxy_url ?? "";
}

export function updateNetworkRoute(settings: AppSettings, mode: NetworkRouteMode, rawUrl: string): AppSettings {
  if (mode === "direct") {
    return { ...settings, network_route: null };
  }

  if (mode === "auto_detect") {
    return {
      ...settings,
      network_route: {
        mode,
        local_proxy_url: null
      }
    };
  }

  const trimmedUrl = rawUrl.trim();
  return {
    ...settings,
    network_route: {
      mode,
      local_proxy_url: trimmedUrl
    }
  };
}
