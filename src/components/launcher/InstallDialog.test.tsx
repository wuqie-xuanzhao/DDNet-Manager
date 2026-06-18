import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { InstallDialog } from "./InstallDialog";
import type { useClientInstaller } from "@/hooks/useClientInstaller";
import type { ClientCatalogEntry, ClientInstallation } from "@/types";

// Mock framer-motion 让动画立即完成，避免 jsdom 下卡住
vi.mock("framer-motion", () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: new Proxy({}, {
    get: () => (props: Record<string, unknown> & { children?: React.ReactNode }) => {
      const { children, ...rest } = props;
      const Comp = "div";
      return <Comp {...(rest as object)}>{children as React.ReactNode}</Comp>;
    }
  })
}));

// Mock 文件浏览器 + probeDisk
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));

vi.mock("@/lib/tauri", () => ({
  probeDisk: vi.fn().mockResolvedValue({
    free_bytes: 1024 * 1024 * 1024 * 100,
    total_bytes: 1024 * 1024 * 1024 * 500,
    is_ssd: true,
    mount_point: "C:\\"
  })
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ hide: vi.fn() })
}));

function makeClient(overrides: Partial<ClientInstallation> = {}): ClientInstallation {
  return {
    id: "client-1",
    client_id: "qmclient",
    display_name: "QmClient",
    install_dir: "C:/Games/QmClient",
    executable_path: "C:/Games/QmClient/DDNet.exe",
    storage_cfg_path: "C:/Games/QmClient/storage.cfg",
    data_dir: "C:/Games/QmClient/data",
    user_data_dir: null,
    version: "1.0.0",
    is_default: false,
    health: "ok",
    missing_items: [],
    install_source: "manual",
    confidence: "verified",
    manager_owned: false,
    compatibility: {
      status: "supported",
      can_launch: true,
      launch_verified: false,
      reasons: [],
      last_launch_result: null
    },
    upstream_url: null,
    pe_company_name: null,
    pe_product_name: null,
    pe_file_version: null,
    exe_sha256: null,
    last_scanned_at: null,
    ...overrides
  };
}

const githubCatalogEntry: ClientCatalogEntry = {
  client_id: "qmclient",
  display_name: "QmClient",
  aliases: ["qmclient"],
  executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
  required_markers: ["storage.cfg", "data"],
  pe_company_names: [],
  pe_product_names: [],
  known_hashes: [],
  update_source: {
    kind: "github_release",
    owner: "wxj881027",
    repo: "QmClient",
    windows_assets: [],
    macos_assets: [],
    linux_assets: []
  },
  upstream_url: "https://github.com/wxj881027/QmClient/releases"
};

const websiteCatalogEntry: ClientCatalogEntry = {
  ...githubCatalogEntry,
  client_id: "cactusclient",
  update_source: { kind: "website", url: "https://cactusss.vercel.app/" }
};

const noneCatalogEntry: ClientCatalogEntry = {
  ...githubCatalogEntry,
  update_source: { kind: "none" },
  upstream_url: null
};

function makeInstaller(overrides: Partial<ReturnType<typeof useClientInstaller>> = {}) {
  return {
    state: {
      kind: "installed" as const,
      version: "1.0.0",
      latest: "1.2.3",
      needsUpdate: false,
      assetSize: 1000000,
      releaseUrl: "https://github.com/wxj881027/QmClient/releases/tag/v1.2.3"
    },
    client: makeClient(),
    clients: [makeClient()],
    catalogEntry: githubCatalogEntry,
    scanning: false,
    installDialogOpen: true,
    installDialogMode: "install" as const,
    buttonProps: { canLaunch: true, disabled: false, hasUpdate: false, latestVersion: "1.2.3", downloading: false, progress: 0, broken: false },
    triggerScan: vi.fn().mockResolvedValue(undefined),
    openInstallDialog: vi.fn(),
    openUpdateDialog: vi.fn(),
    closeInstallDialog: vi.fn(),
    beginInstall: vi.fn().mockResolvedValue(undefined),
    launchGame: vi.fn().mockResolvedValue(undefined),
    selectClient: vi.fn(),
    refreshFromRegistry: vi.fn().mockResolvedValue(undefined),
    fetchRelease: vi.fn().mockResolvedValue(undefined),
    ...overrides
  } as ReturnType<typeof useClientInstaller>;
}

describe("InstallDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => cleanup());

  it("installDialogOpen=false 时不渲染弹窗内容", () => {
    const installer = makeInstaller({ installDialogOpen: false });
    const { container } = render(<InstallDialog installer={installer} displayName="QmClient" gameId="qmclient" />);
    // radix Dialog 在 open=false 时不渲染 portal 内容，DOM 里没有"安装 QmClient"
    expect(container.textContent).not.toContain("安装 QmClient");
  });

  it("github_release 源：显示真实版本号 + 体积 + GitHub Release 链接", () => {
    const installer = makeInstaller();
    render(<InstallDialog installer={installer} displayName="QmClient" gameId="qmclient" />);
    expect(screen.getByText("v1.2.3")).toBeInTheDocument();
    // 1000000 bytes = 976.6 KB（toFixed(1)）；radix Dialog portal 渲染到 body
    expect(document.body.textContent).toMatch(/976\.6 KB/);
    expect(screen.getByText("GitHub Release")).toHaveAttribute("href", "https://github.com/wxj881027/QmClient/releases/tag/v1.2.3");
  });

  it("website 源：显示'打开官网下载'按钮，不显示版本卡（review #13）", () => {
    const installer = makeInstaller({
      catalogEntry: websiteCatalogEntry,
      state: { kind: "installed", version: null, latest: null, needsUpdate: false, assetSize: null, releaseUrl: null }
    });
    render(<InstallDialog installer={installer} displayName="Cactus Client" gameId="cactusclient" />);
    expect(screen.getByText("打开官网下载")).toHaveAttribute("href", "https://cactusss.vercel.app/");
    // 不应该显示 GitHub Release 链接（website 源不显示版本卡）
    expect(screen.queryByText("GitHub Release")).not.toBeInTheDocument();
  });

  it("none 源：显示'暂无自动下载源'提示", () => {
    const installer = makeInstaller({
      catalogEntry: noneCatalogEntry,
      state: { kind: "installed", version: null, latest: null, needsUpdate: false, assetSize: null, releaseUrl: null }
    });
    render(<InstallDialog installer={installer} displayName="Unknown" gameId="unknown" />);
    expect(document.body.textContent).toMatch(/暂无自动下载源/);
  });

  it("点击'开始安装'调用 beginInstall", () => {
    const installer = makeInstaller();
    render(<InstallDialog installer={installer} displayName="QmClient" gameId="qmclient" />);
    fireEvent.click(screen.getByRole("button", { name: /开始安装/ }));
    expect(installer.beginInstall).toHaveBeenCalledTimes(1);
    const arg = (installer.beginInstall as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(arg).toMatchObject({ desktop: true, startMenu: true });
    expect(arg.installDir).toContain("qmclient");
  });

  it("点击'已安装？定位游戏'调用 triggerScan", () => {
    const installer = makeInstaller();
    render(<InstallDialog installer={installer} displayName="QmClient" gameId="qmclient" />);
    fireEvent.click(screen.getByText("已安装？定位游戏"));
    expect(installer.triggerScan).toHaveBeenCalledTimes(1);
  });

  it("update 模式标题为'更新 {name}'，跳过快捷方式 checkbox", () => {
    const installer = makeInstaller({ installDialogMode: "update" });
    render(<InstallDialog installer={installer} displayName="QmClient" gameId="qmclient" />);
    expect(screen.getByText("更新 QmClient")).toBeInTheDocument();
    // update 模式不渲染快捷方式 checkbox
    expect(screen.queryByText("创建桌面快捷方式")).not.toBeInTheDocument();
  });
});
