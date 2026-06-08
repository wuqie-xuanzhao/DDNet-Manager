import { describe, expect, it } from "vitest";
import { defaultAppSettings, networkRouteUrl, updateNetworkRoute } from "./settings";

describe("updateNetworkRoute", () => {
  it("clears route settings in direct mode", () => {
    const next = updateNetworkRoute(
      {
        ...defaultAppSettings,
        network_route: {
          mode: "proxy_prefix",
          proxy_prefix_url: "https://proxy.example/",
          mirror_template: null,
          enabled_hosts: ["proxy.example"]
        }
      },
      "direct",
      "https://proxy.example/"
    );

    expect(next.network_route).toBeNull();
  });

  it("derives enabled host for proxy routes", () => {
    const next = updateNetworkRoute(defaultAppSettings, "proxy_prefix", " https://proxy.example/path ");

    expect(next.network_route).toEqual({
      mode: "proxy_prefix",
      proxy_prefix_url: "https://proxy.example/path",
      mirror_template: null,
      enabled_hosts: ["proxy.example"]
    });
    expect(networkRouteUrl(next)).toBe("https://proxy.example/path");
  });
});
