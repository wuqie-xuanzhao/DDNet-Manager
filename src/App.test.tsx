import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { defaultAppSettings } from "./lib/settings";

const changeSettings = vi.fn();
const handleBrowse = vi.fn();
const handleClientPathChange = vi.fn();
const handlePrimaryAction = vi.fn();
const handleValidate = vi.fn();
const minimizeWindow = vi.fn();
const closeWindow = vi.fn();
const saveSettings = vi.fn();
const selectClientType = vi.fn();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: closeWindow,
    minimize: minimizeWindow
  })
}));

vi.mock("./lib/tauri", () => ({
  isTauriRuntime: () => false
}));

vi.mock("./hooks/useAppSettings", () => ({
  useAppSettings: () => ({
    appSettings: defaultAppSettings,
    savedAppSettings: defaultAppSettings,
    settingsError: null,
    settingsState: "idle",
    changeSettings,
    saveSettings
  })
}));

vi.mock("./hooks/useAutoUpdate", () => ({
  useAutoUpdate: () => ({
    autoUpdate: null,
    autoUpdateError: null,
    autoUpdateState: "disabled"
  })
}));

vi.mock("./hooks/useClientLauncher", () => ({
  useClientLauncher: () => ({
    clientPath: "",
    errorMessage: null,
    handleBackgroundValidationError: vi.fn(),
    handleBrowse,
    handleClientPathChange,
    handlePrimaryAction,
    handleValidate,
    launchReadiness: {
      client: null,
      can_launch: false,
      running: false,
      status_label: "未设置",
      user_message: "尚未设置默认客户端，请先定位并保存一个客户端。",
      blocking_reasons: ["没有默认客户端记录"],
      checked_at: null
    },
    launcherState: "unconfigured",
    selectedClient: null,
    selectedClientTypeId: "qmclient",
    selectClientType
  })
}));

describe("App launcher shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the DDNet launcher shell", () => {
    render(<App />);

    expect(document.querySelector("#ddnet-launcher")).toBeInTheDocument();
    expect(screen.getByLabelText("DDNet Manager 首页")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "全部游戏" })).toBeInTheDocument();
    expect(screen.getAllByText("QmClient").length).toBeGreaterThan(0);
  });

  it("keeps DDNet community and website actions in the launcher social rail", () => {
    render(<App />);

    expect(screen.getByRole("link", { name: "加入 QmClient QQ 群" })).toHaveAttribute("href", expect.stringContaining("1076765929"));
    expect(screen.getByRole("link", { name: "访问 DDRace Workshop" })).toHaveAttribute("href", "https://ddrace.cn/");
    expect(screen.getByRole("link", { name: "访问 DDNet 官方网站" })).toHaveAttribute("href", "https://ddnet.org/");
  });

  it("opens the recreated launcher settings panel", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    expect(screen.getByRole("button", { name: "下载" })).toBeInTheDocument();
    expect(screen.getByText("开机自动运行 DDNet Manager")).toBeInTheDocument();
  });

  it("lets users click a game card from the library list", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "全部游戏" }));
    fireEvent.click(screen.getAllByRole("button", { name: "选择 DDNet" })[0]);

    expect(screen.getByText("官方发行版")).toBeInTheDocument();
  });

  it("keeps the exit confirmation dialog above its backdrop", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "关闭" }));

    const backdrop = document.querySelector("#exit-modal-backdrop");
    const content = document.querySelector("#exit-modal-content");

    expect(screen.getByText("关闭启动器")).toBeInTheDocument();
    expect(backdrop).toHaveClass("z-0");
    expect(content).toHaveClass("z-10");
  });

  it("minimizes the window when confirming the default close action", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    fireEvent.click(screen.getByRole("button", { name: "确 认" }));

    expect(minimizeWindow).toHaveBeenCalledTimes(1);
    expect(closeWindow).not.toHaveBeenCalled();
  });
});
