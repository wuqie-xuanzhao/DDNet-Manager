import type { AppSettings, NetworkRouteMode } from "../types";

export const defaultAppSettings: AppSettings = {
  network_route: null,
  scan_excluded_paths: [],
  use_everything: false,
  close_panel_after_launch: true,
  auto_check_updates: false,
  advanced_manifest_url: null
};

export function routeHostFromUrl(value: string) {
  try {
    return new URL(value).hostname;
  } catch {
    return "";
  }
}

export function networkRouteUrl(settings: AppSettings) {
  return settings.network_route?.proxy_prefix_url ?? settings.network_route?.mirror_template ?? "";
}

export function updateNetworkRoute(settings: AppSettings, mode: NetworkRouteMode, rawUrl: string): AppSettings {
  const trimmedUrl = rawUrl.trim();
  if (mode === "direct") {
    return { ...settings, network_route: null };
  }

  const host = routeHostFromUrl(trimmedUrl);
  return {
    ...settings,
    network_route: {
      mode,
      proxy_prefix_url: mode === "proxy_prefix" ? trimmedUrl : null,
      mirror_template: mode === "mirror_template" ? trimmedUrl : null,
      enabled_hosts: host ? [host] : []
    }
  };
}
