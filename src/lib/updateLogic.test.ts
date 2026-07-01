import { describe, expect, it } from "vitest";
import {
  buildNetworkRoute,
  buildUpdateSourceRequest,
  deriveAutoUpdateView,
  networkRouteHint,
  networkRouteLabel,
  networkRoutePlaceholder,
  progressPercent,
  resolveUpdateManifestInput
} from "./updateLogic";
import type { ClientUpdateCheck, DownloadJob } from "../types";

const baseJob: DownloadJob = {
  id: "download-1",
  client_installation_id: "qmclient-main",
  client_id: "qmclient",
  channel: "stable",
  version: "2.62.4",
  asset_url: "https://example.com/QmClient-windows.zip",
  sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  size: 100,
  status: "downloading",
  downloaded_bytes: 25,
  cache_path: "D:/Cache/download-1.zip",
  error: null
};

describe("progressPercent", () => {
  it("rounds and caps download progress", () => {
    expect(progressPercent(baseJob)).toBe(25);
    expect(progressPercent({ ...baseJob, downloaded_bytes: 140 })).toBe(100);
  });

  it("returns zero when size is unknown", () => {
    expect(progressPercent({ ...baseJob, size: 0 })).toBe(0);
  });
});

describe("buildNetworkRoute", () => {
  it("returns null for direct mode", () => {
    expect(buildNetworkRoute("direct", "")).toBeNull();
  });

  it("keeps route mode labels stable", () => {
    expect(networkRouteLabel("direct")).toBe("直接下载");
    expect(networkRouteLabel("auto_detect")).toBe("自动检测");
    expect(networkRouteLabel("local_proxy")).toBe("手动填写");
  });

  it("exposes user-facing hint text explaining each route mode", () => {
    expect(networkRouteHint("direct")).toContain("github.com");
    expect(networkRouteHint("local_proxy")).toContain("127.0.0.1:7890");
  });

  it("provides a common local proxy placeholder example", () => {
    expect(networkRoutePlaceholder("direct")).toBe("");
    expect(networkRoutePlaceholder("local_proxy")).toContain("127.0.0.1");
  });

  it("throws for empty non-direct routes", () => {
    expect(() => buildNetworkRoute("local_proxy", "   ")).toThrow("route_url_invalid");
  });

  it("builds a local proxy route from a trimmed url", () => {
    expect(buildNetworkRoute("local_proxy", " http://127.0.0.1:7890 ")).toEqual({
      mode: "local_proxy",
      local_proxy_url: "http://127.0.0.1:7890"
    });
  });

  it("builds an auto_detect route without requiring a url", () => {
    expect(buildNetworkRoute("auto_detect", "")).toEqual({
      mode: "auto_detect",
      local_proxy_url: null
    });
  });

  it("exposes label and hint for auto_detect mode", () => {
    expect(networkRouteLabel("auto_detect")).toBe("自动检测");
    expect(networkRouteHint("auto_detect")).toContain("系统代理");
    expect(networkRoutePlaceholder("auto_detect")).toBe("");
  });
});

describe("deriveAutoUpdateView", () => {
  const update: ClientUpdateCheck = {
    client_id: "qmclient",
    channel: "stable",
    current_version: "2.62.3",
    latest_version: "2.62.4",
    asset: {
      platform: "windows-x86_64",
      asset_url: "https://example.com/QmClient.zip",
      sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      size: 100
    },
    needs_update: true,
    source_kind: "manifest",
    action: "download",
    action_url: null,
    message: null,
    reason: "none"
  };

  it("hides stale update results when the active request key changes", () => {
    const view = deriveAutoUpdateView({
      mode: "ready",
      requestKey: "client-b",
      snapshot: {
        requestKey: "client-a",
        update,
        error: null
      }
    });

    expect(view).toEqual({
      autoUpdate: null,
      autoUpdateError: null,
      autoUpdateState: "checking"
    });
  });

  it("treats non-none reason as error instead of current to avoid false latest", () => {
    // 回归守卫：旧逻辑把 checkClientUpdate 返回的 null（表示检查不可用）误判为
    // "current"（已是最新版）。新逻辑用 reason != "none" 表示检查不可用，
    // 必须映射到 error state 并展示后端 message，不能误判为"已是最新版"。
    const unavailable: ClientUpdateCheck = {
      ...update,
      action: "none",
      needs_update: false,
      latest_version: "",
      message: "该渠道下无匹配的 release。",
      reason: "no_release_for_channel"
    };
    const view = deriveAutoUpdateView({
      mode: "ready",
      requestKey: "client-a",
      snapshot: {
        requestKey: "client-a",
        update: unavailable,
        error: null
      }
    });

    expect(view.autoUpdateState).toBe("error");
    expect(view.autoUpdate).toBeNull();
    expect(view.autoUpdateError).toBe("该渠道下无匹配的 release。");
  });

  it("maps all non-none reason variants to error state", () => {
    // 覆盖多个 reason 值，确保枚举字符串拼接无拼写错误，且前端对所有非 none 值行为一致。
    const reasons: Array<{ reason: ClientUpdateCheck["reason"]; message: string }> = [
      { reason: "client_not_in_catalog", message: "客户端不在内置 catalog 中。" },
      { reason: "no_asset_for_platform", message: "无当前平台的匹配资产。" },
      { reason: "auto_update_disabled", message: "该客户端不支持自动更新。" },
      { reason: "manifest_entry_missing", message: "manifest 中无该条目。" }
    ];
    for (const { reason, message } of reasons) {
      const unavailable: ClientUpdateCheck = {
        ...update,
        action: "none",
        needs_update: false,
        latest_version: "",
        message,
        reason
      };
      const view = deriveAutoUpdateView({
        mode: "ready",
        requestKey: "client-a",
        snapshot: {
          requestKey: "client-a",
          update: unavailable,
          error: null
        }
      });
      expect(view.autoUpdateState).toBe("error");
      expect(view.autoUpdateError).toBe(message);
    }
  });

  it("maps reason none with needs_update false to current (genuine latest)", () => {
    const latest: ClientUpdateCheck = {
      ...update,
      needs_update: false,
      latest_version: "2.62.3",
      reason: "none"
    };
    const view = deriveAutoUpdateView({
      mode: "ready",
      requestKey: "client-a",
      snapshot: {
        requestKey: "client-a",
        update: latest,
        error: null
      }
    });

    expect(view.autoUpdateState).toBe("current");
  });
});

describe("resolveUpdateManifestInput", () => {
  it("uses smoke manifest input without waiting for state synchronization", () => {
    expect(
      resolveUpdateManifestInput({
        smokeEnabled: true,
        smokeManifestUrl: "https://example.com/smoke.json"
      })
    ).toEqual({
      useManifestSource: true,
      manifestUrl: "https://example.com/smoke.json"
    });
  });

  it("builds a catalog update request without manifest source", () => {
    expect(
      buildUpdateSourceRequest({
        clientId: "qmclient",
        channel: "stable",
        manifestUrl: "https://example.com/manifest.json",
        routeMode: "direct",
        routeUrl: "",
        useManifestSource: false
      })
    ).toEqual({
      client_id: "qmclient",
      channel: "stable",
      manifest_url: null,
      network_route: null,
      use_manifest_source: false
    });
  });

  it("builds a manifest update request with a configured local proxy route", () => {
    expect(
      buildUpdateSourceRequest({
        clientId: "qmclient",
        channel: "stable",
        manifestUrl: " https://example.com/manifest.json ",
        routeMode: "local_proxy",
        routeUrl: " http://127.0.0.1:7890 ",
        useManifestSource: true
      })
    ).toEqual({
      client_id: "qmclient",
      channel: "stable",
      manifest_url: "https://example.com/manifest.json",
      network_route: {
        mode: "local_proxy",
        local_proxy_url: "http://127.0.0.1:7890"
      },
      use_manifest_source: true
    });
  });
});
