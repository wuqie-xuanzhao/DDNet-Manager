import type {
  CheckClientUpdateRequest,
  ClientUpdateCheck,
  DownloadJob,
  NetworkRouteConfig,
  NetworkRouteMode,
  StartUpdateDownloadRequest
} from "../types";

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

  if (routeMode === "auto_detect") {
    return {
      mode: routeMode,
      local_proxy_url: null
    };
  }

  const trimmedUrl = routeUrl.trim();
  if (!trimmedUrl) {
    throw new Error("route_url_invalid");
  }

  return {
    mode: routeMode,
    local_proxy_url: trimmedUrl
  };
}

export function networkRouteLabel(mode: NetworkRouteMode) {
  switch (mode) {
    case "direct":
      return "直接下载";
    case "auto_detect":
      return "自动检测";
    case "local_proxy":
      return "手动填写";
  }
}

/** 本地代理地址输入框的 placeholder，给出常见本地代理端口示例。 */
export function networkRoutePlaceholder(mode: NetworkRouteMode): string {
  switch (mode) {
    case "direct":
      return "";
    case "auto_detect":
      return "";
    case "local_proxy":
      return "http://127.0.0.1:7890";
  }
}

/** 网络路由模式的用户引导文案：用白话解释行为，帮助用户理解直接下载与本地代理的差异。 */
export function networkRouteHint(mode: NetworkRouteMode): string {
  switch (mode) {
    case "direct":
      return "直接访问 github.com 与 api.github.com。若你的网络无法访问 GitHub，请改用本地代理。";
    case "auto_detect":
      return "自动检测系统代理（环境变量 HTTPS_PROXY 或 Windows 系统代理）。适合已配置 Clash/V2Ray 系统代理的用户，无需手动填写地址。";
    case "local_proxy":
      return "通过本地代理（如 Clash、v2ray 的本地端口）访问 GitHub。填写你的代理地址，常见为 http://127.0.0.1:7890。所有下载与更新请求都会走这个代理隧道。";
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
    // 兜底：catch 分支已由 error 优先处理，正常成功路径 update 一定非 null。
    // 到这里说明状态异常，按 error 展示而非误判为"已是最新版"。
    return {
      autoUpdate: null,
      autoUpdateError: "更新检查未返回有效结果。",
      autoUpdateState: "error" as const
    };
  }

  // reason != "none" 表示检查不可用（catalog 无条目/无 release/无资产/不支持自动更新/manifest 无条目）。
  // 旧逻辑把这些情况误判为 "current"（已是最新版），这里改为展示后端 message 的 error state。
  if (input.snapshot.update.reason !== "none") {
    return {
      autoUpdate: null,
      autoUpdateError: input.snapshot.update.message ?? "当前客户端无法检查自动更新。",
      autoUpdateState: "error" as const
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
}) {
  if (input.smokeEnabled) {
    return {
      useManifestSource: true,
      manifestUrl: input.smokeManifestUrl
    };
  }

  return {
    useManifestSource: false,
    manifestUrl: ""
  };
}
