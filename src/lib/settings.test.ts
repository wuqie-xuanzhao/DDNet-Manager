import { describe, expect, it } from "vitest";
import { defaultAppSettings, networkRouteUrl, updateNetworkRoute } from "./settings";

describe("updateNetworkRoute", () => {
  it("clears route settings in direct mode", () => {
    const next = updateNetworkRoute(
      {
        ...defaultAppSettings,
        network_route: {
          mode: "local_proxy",
          local_proxy_url: "http://127.0.0.1:7890"
        }
      },
      "direct",
      "http://127.0.0.1:7890"
    );

    expect(next.network_route).toBeNull();
  });

  it("stores the trimmed local proxy url for local proxy mode", () => {
    const next = updateNetworkRoute(defaultAppSettings, "local_proxy", " http://127.0.0.1:7890 ");

    expect(next.network_route).toEqual({
      mode: "local_proxy",
      local_proxy_url: "http://127.0.0.1:7890"
    });
    expect(networkRouteUrl(next)).toBe("http://127.0.0.1:7890");
  });

  it("stores auto_detect mode without requiring a proxy url", () => {
    const next = updateNetworkRoute(defaultAppSettings, "auto_detect", "http://127.0.0.1:7890");

    expect(next.network_route).toEqual({
      mode: "auto_detect",
      local_proxy_url: null
    });
  });
});
