import type { ClientUpdateCheck, DownloadJob, NetworkRouteConfig, NetworkRouteMode } from "../types";
import { routeHostFromUrl } from "./settings";

export type AutoUpdateViewState = "disabled" | "idle" | "checking" | "available" | "current" | "manual" | "error";

export type AutoUpdateMode = "disabled" | "idle" | "loading" | "ready";

export type AutoUpdateSnapshot = {
  requestKey: string;
  update: ClientUpdateCheck | null;
  error: string | null;
};

export function progressPercent(job: DownloadJob | null) {
  if (!job || job.size === 0) {
    return 0;
  }

  return Math.min(100, Math.round((job.downloaded_bytes / job.size) * 100));
}

export function buildNetworkRoute(routeMode: NetworkRouteMode, routeUrl: string): NetworkRouteConfig | null {
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
}

export function deriveAutoUpdateView(input: {
  mode: AutoUpdateMode;
  requestKey: string | null;
  snapshot: AutoUpdateSnapshot | null;
}) {
  if (input.mode === "disabled") {
    return {
      autoUpdate: null,
      autoUpdateError: null,
      autoUpdateState: "disabled" as const
    };
  }

  if (input.mode === "idle") {
    return {
      autoUpdate: null,
      autoUpdateError: null,
      autoUpdateState: "idle" as const
    };
  }

  if (input.mode === "loading" || !input.requestKey || input.snapshot?.requestKey !== input.requestKey) {
    return {
      autoUpdate: null,
      autoUpdateError: null,
      autoUpdateState: "checking" as const
    };
  }

  if (input.snapshot.error) {
    return {
      autoUpdate: null,
      autoUpdateError: input.snapshot.error,
      autoUpdateState: "error" as const
    };
  }

  if (!input.snapshot.update) {
    return {
      autoUpdate: null,
      autoUpdateError: null,
      autoUpdateState: "current" as const
    };
  }

  return {
    autoUpdate: input.snapshot.update,
    autoUpdateError: null,
    autoUpdateState:
      input.snapshot.update.action === "open_url"
        ? ("manual" as const)
        : input.snapshot.update.needs_update
          ? ("available" as const)
          : ("current" as const)
  };
}

export function resolveUpdateManifestInput(input: {
  smokeEnabled: boolean;
  smokeManifestUrl: string;
  useManifestSource: boolean;
  manifestUrl: string;
}) {
  if (input.smokeEnabled) {
    return {
      useManifestSource: true,
      manifestUrl: input.smokeManifestUrl
    };
  }

  return {
    useManifestSource: input.useManifestSource,
    manifestUrl: input.manifestUrl
  };
}
