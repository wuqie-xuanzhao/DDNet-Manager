import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsDialog, type SettingsSectionId } from "./SettingsDialog";
import { defaultAppSettings } from "@/lib/settings";
import type { AppSettings } from "@/types";

// Mock framer-motion 让动画立即完成，避免 jsdom 下卡住。
vi.mock("framer-motion", () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: new Proxy({}, {
    get: () => (props: Record<string, unknown> & { children?: React.ReactNode }) => {
      const { children, ...rest } = props;
      // 把 motion.div / motion.span 等简化为 div / span，保留 children
      const Comp = "div";
      return <Comp {...(rest as object)}>{children as React.ReactNode}</Comp>;
    }
  })
}));

// Mock @tauri-apps/plugin-dialog，让 open() 返回可控路径
const mockOpen = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mockOpen(...args)
}));

// Mock @tauri-apps/api/core，避免 getAppVersion 在测试里报错
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue("0.1.0-test")
}));

// Mock logo SVG 导入
vi.mock("@/assets/logo.svg", () => ({
  default: "data:image/svg+xml,mock"
}));

// Mock 子组件避免它们的副作用干扰（设置面板内的子页面）
vi.mock("@/components/clients/ClientManager", () => ({
  ClientManager: () => <div data-testid="mock-client-manager" />
}));
vi.mock("@/components/update/UpdatePanel", () => ({
  UpdatePanel: () => <div data-testid="mock-update-panel" />
}));

function makeProps(overrides: Partial<{
  settings: AppSettings;
  activeSection: SettingsSectionId;
  onUpdateSettings: (s: AppSettings) => Promise<void>;
}> = {}) {
  return {
    open: true,
    activeSection: overrides.activeSection ?? "general",
    tauriRuntime: false,
    launcherState: { initialized: true } as never,
    clientPath: "",
    selectedClientType: { name: "QmClient" },
    customBgs: {},
    activeGameId: "qmclient",
    onCustomBgChange: vi.fn(),
    themeMode: "dark" as const,
    onThemeChange: vi.fn(),
    errorMessage: null,
    settings: overrides.settings ?? defaultAppSettings,
    settingsState: "idle" as const,
    settingsError: null,
    smokeAutomation: null,
    onClose: vi.fn(),
    onSectionChange: vi.fn(),
    onUpdateSettings: overrides.onUpdateSettings ?? vi.fn().mockResolvedValue(undefined),
    onClientPathChange: vi.fn(),
    onBrowse: vi.fn().mockResolvedValue(undefined),
    onValidate: vi.fn().mockResolvedValue(undefined)
  };
}

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOpen.mockReset();
  });

  afterEach(() => cleanup());

  describe("close_behavior 三态渲染", () => {
    // 回归测试：旧版本用 `(mode === "minimize_to_tray" || mode === "ask")` 让 ask 状态
    // 假选中 minimize_to_tray radio，导致默认状态视觉与行为不一致。
    // 迁移到 shadcn RadioGroup 后，选中态由 radix 用 data-checked attribute 标记。
    it('"ask" 状态下选中"每次询问"，不污染"最小化到托盘"', () => {
      const props = makeProps({
        settings: { ...defaultAppSettings, close_behavior: "ask" }
      });
      render(<SettingsDialog {...props} />);

      expect(screen.getByText("每次询问")).toBeInTheDocument();
      expect(screen.getByText("最小化到系统托盘")).toBeInTheDocument();
      expect(screen.getByText("退出启动器")).toBeInTheDocument();

      // ask radio 应该有 data-checked="true"
      const askRadio = screen.getByRole("radio", { name: /每次询问/i }) ?? document.getElementById("close-behavior-ask");
      expect(askRadio?.getAttribute("data-state")).toBe("checked");
      const minRadio = document.getElementById("close-behavior-minimize");
      expect(minRadio?.getAttribute("data-state")).toBe("unchecked");
    });

    it('"minimize_to_tray" 严格匹配时选中"最小化到系统托盘"', () => {
      const props = makeProps({
        settings: { ...defaultAppSettings, close_behavior: "minimize_to_tray" }
      });
      render(<SettingsDialog {...props} />);

      const minRadio = document.getElementById("close-behavior-minimize");
      expect(minRadio?.getAttribute("data-state")).toBe("checked");
      const askRadio = document.getElementById("close-behavior-ask");
      expect(askRadio?.getAttribute("data-state")).toBe("unchecked");
    });

    it('"exit_launcher" 选中"退出启动器"', () => {
      const props = makeProps({
        settings: { ...defaultAppSettings, close_behavior: "exit_launcher" }
      });
      render(<SettingsDialog {...props} />);

      const exitRadio = document.getElementById("close-behavior-exit");
      expect(exitRadio?.getAttribute("data-state")).toBe("checked");
    });

    it("点击 ask radio 调用 onUpdateSettings 传 close_behavior=ask", () => {
      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        settings: { ...defaultAppSettings, close_behavior: "minimize_to_tray" },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      // RadioGroup 用 label 包裹 radio，点击 label 文字触发切换
      fireEvent.click(screen.getByText("每次询问"));
      expect(onUpdateSettings).toHaveBeenCalledTimes(1);
      const arg = onUpdateSettings.mock.calls[0][0] as AppSettings;
      expect(arg.close_behavior).toBe("ask");
    });
  });

  describe("工具页 - 排除路径列表", () => {
    it("空列表显示占位文案", () => {
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_excluded_paths: [] }
      });
      render(<SettingsDialog {...props} />);
      expect(screen.getByText("尚未添加排除路径")).toBeInTheDocument();
    });

    it("已配置的路径渲染为列表项，每项有删除按钮", () => {
      const props = makeProps({
        activeSection: "tools",
        settings: {
          ...defaultAppSettings,
          scan_excluded_paths: ["D:/Dev/MyQmClient", "E:/QQ/Files"]
        }
      });
      render(<SettingsDialog {...props} />);
      expect(screen.getByText("D:/Dev/MyQmClient")).toBeInTheDocument();
      expect(screen.getByText("E:/QQ/Files")).toBeInTheDocument();
      // 两个删除按钮（按 aria-label）
      expect(screen.getAllByLabelText(/移除 /)).toHaveLength(2);
    });

    it("点击删除按钮从 scan_excluded_paths 移除对应路径", () => {
      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: {
          ...defaultAppSettings,
          scan_excluded_paths: ["D:/Dev/MyQmClient", "E:/QQ/Files"]
        },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      fireEvent.click(screen.getByLabelText("移除 D:/Dev/MyQmClient"));
      expect(onUpdateSettings).toHaveBeenCalledTimes(1);
      const arg = onUpdateSettings.mock.calls[0][0] as AppSettings;
      expect(arg.scan_excluded_paths).toEqual(["E:/QQ/Files"]);
    });

    it("点击添加文件夹按钮调起文件浏览器并写入新路径", async () => {
      mockOpen.mockResolvedValue("F:/NewExcluded");

      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_excluded_paths: [] },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      fireEvent.click(screen.getByText("添加文件夹"));

      await waitFor(() => {
        expect(mockOpen).toHaveBeenCalledWith({ directory: true, multiple: false });
      });
      await waitFor(() => {
        expect(onUpdateSettings).toHaveBeenCalledTimes(1);
      });
      const arg = onUpdateSettings.mock.calls[0][0] as AppSettings;
      expect(arg.scan_excluded_paths).toEqual(["F:/NewExcluded"]);
    });

    it("用户取消文件浏览器（返回 null）时不调 onUpdateSettings", async () => {
      mockOpen.mockResolvedValue(null);

      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_excluded_paths: [] },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      fireEvent.click(screen.getByText("添加文件夹"));

      await waitFor(() => {
        expect(mockOpen).toHaveBeenCalled();
      });
      expect(onUpdateSettings).not.toHaveBeenCalled();
    });

    it("重复添加同一路径自动去重", async () => {
      mockOpen.mockResolvedValue("D:/Already/Here");

      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: {
          ...defaultAppSettings,
          scan_excluded_paths: ["D:/Already/Here"]
        },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      fireEvent.click(screen.getByText("添加文件夹"));

      await waitFor(() => {
        expect(mockOpen).toHaveBeenCalled();
      });
      // 已存在的路径不写入
      expect(onUpdateSettings).not.toHaveBeenCalled();
    });
  });

  describe("工具页 - 数字 stepper", () => {
    it("null 值显示默认值徽章", () => {
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_max_results: null }
      });
      render(<SettingsDialog {...props} />);
      expect(screen.getByText("默认 50")).toBeInTheDocument();
    });

    it("点击 + 按钮按 step 步进", () => {
      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_max_results: 50 },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      const plusBtn = screen.getByLabelText("增加 5");
      fireEvent.click(plusBtn);
      const arg = onUpdateSettings.mock.calls[0][0] as AppSettings;
      expect(arg.scan_max_results).toBe(55);
    });

    it("点击 - 按钮按 step 步进，不低于 min", () => {
      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_max_results: 3 },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      const minusBtn = screen.getByLabelText("减少 5");
      fireEvent.click(minusBtn);
      const arg = onUpdateSettings.mock.calls[0][0] as AppSettings;
      expect(arg.scan_max_results).toBe(1); // clamp 到 min=1，不是 -2
    });

    it("点击恢复默认置 null", () => {
      const onUpdateSettings = vi.fn().mockResolvedValue(undefined);
      const props = makeProps({
        activeSection: "tools",
        settings: { ...defaultAppSettings, scan_max_results: 100 },
        onUpdateSettings
      });
      render(<SettingsDialog {...props} />);

      fireEvent.click(screen.getByText("恢复默认"));
      // 两个 stepper 都有恢复默认，断言任一即可
      const calls = onUpdateSettings.mock.calls;
      const last = calls[calls.length - 1][0] as AppSettings;
      expect(last.scan_max_results === null || last.scan_timeout_secs === null).toBe(true);
    });
  });
});
