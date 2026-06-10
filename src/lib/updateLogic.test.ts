import { describe, expect, it } from "vitest";
import {
  buildNetworkRoute,
  buildUpdateSourceRequest,
  deriveAutoUpdateView,
  networkRouteLabel,
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
    expect(networkRouteLabel("proxy_prefix")).toBe("代理前缀");
    expect(networkRouteLabel("mirror_template")).toBe("镜像模板");
  });

  it("throws for invalid non-direct routes", () => {
    expect(() => buildNetworkRoute("mirror_template", "not a url")).toThrow("route_url_invalid");
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
    message: null
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
});

describe("resolveUpdateManifestInput", () => {
  it("uses smoke manifest input without waiting for state synchronization", () => {
    expect(
      resolveUpdateManifestInput({
        smokeEnabled: true,
        smokeManifestUrl: "https://example.com/smoke.json",
        useManifestSource: false,
        manifestUrl: ""
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

  it("builds a manifest update request with a configured mirror route", () => {
    expect(
      buildUpdateSourceRequest({
        clientId: "qmclient",
        channel: "stable",
        manifestUrl: " https://example.com/manifest.json ",
        routeMode: "mirror_template",
        routeUrl: "https://mirror.example.com/{url}",
        useManifestSource: true
      })
    ).toEqual({
      client_id: "qmclient",
      channel: "stable",
      manifest_url: "https://example.com/manifest.json",
      network_route: {
        mode: "mirror_template",
        proxy_prefix_url: null,
        mirror_template: "https://mirror.example.com/{url}",
        enabled_hosts: ["mirror.example.com"]
      },
      use_manifest_source: true
    });
  });
});
