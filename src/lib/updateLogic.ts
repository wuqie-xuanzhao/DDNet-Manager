import type {
  CheckClientUpdateRequest,
  ClientUpdateCheck,
  DownloadJob,
  NetworkRouteConfig,
  NetworkRouteMode,
  StartUpdateDownloadRequest
} from "../types";
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

export function networkRouteLabel(mode: NetworkRouteMode) {
  switch (mode) {
    case "direct":
      return "直接下载";
    case "proxy_prefix":
      return "代理前缀";
    case "mirror_template":
      return "镜像模板";
  }
}

/** 网络路由模式输入框的 placeholder，统一为格式示例（不硬编码实际公共代理）。 */
export function networkRoutePlaceholder(mode: NetworkRouteMode): string {
  switch (mode) {
    case "direct":
      return "";
    case "proxy_prefix":
      return "https://你的代理域名/";
    case "mirror_template":
      return "https://镜像站/path?url={url}";
  }
}

/** 网络路由模式的用户引导文案：解释行为、URL 格式与示例，帮助用户理解差异。 */
export function networkRouteHint(mode: NetworkRouteMode): string {
  switch (mode) {
    case "direct":
      return "直接访问 github.com 与 api.github.com。若你的网络无法访问 GitHub，请改用代理前缀或镜像模板。";
    case "proxy_prefix":
      return "在原始下载地址前拼接此前缀。例如原始 https://github.com/... 会拼接为 https://你的代理域名/https://github.com/... 。需自行提供可用的 HTTPS 代理。";
    case "mirror_template":
      return "用含 {url} 占位符的模板构造访问地址，{url} 会被替换为原始下载地址。需自行提供可用的 HTTPS 镜像。";
  }
}

export function buildUpdateSourceRequest(input: {
  clientId: string;
  channel: string;
  manifestUrl: string;
  routeMode: NetworkRouteMode;
  routeUrl: string;
  useManifestSource: boolean;
}): CheckClientUpdateRequest {
  return {
    client_id: input.clientId,
    channel: input.channel,
    manifest_url: input.useManifestSource ? input.manifestUrl.trim() : null,
    network_route: buildNetworkRoute(input.routeMode, input.routeUrl),
    use_manifest_source: input.useManifestSource
  };
}

export function buildStartUpdateDownloadRequest(input: {
  clientInstallationId: string;
  channel: string;
  manifestUrl: string;
  routeMode: NetworkRouteMode;
  routeUrl: string;
  useManifestSource: boolean;
}): StartUpdateDownloadRequest {
  return {
    client_installation_id: input.clientInstallationId,
    channel: input.channel,
    manifest_url: input.useManifestSource ? input.manifestUrl.trim() : null,
    network_route: buildNetworkRoute(input.routeMode, input.routeUrl),
    use_manifest_source: input.useManifestSource
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
