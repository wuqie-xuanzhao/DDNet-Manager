import { AnimatePresence, motion } from "framer-motion";
import { X, Loader2, ArrowUpRight, CheckCircle2, AlertCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ClientManager } from "@/components/clients/ClientManager";
import { UpdatePanel } from "@/components/update/UpdatePanel";

import { getAppVersion, checkAppUpdate } from "@/lib/tauri";
import type { AppSettings, LauncherState, LocalSmokeAutomationConfig, AppUpdateCheck } from "@/types";
import logoMark from "@/assets/logo.svg";

export type SettingsSectionId = "general" | "clients" | "download" | "updates" | "appearance" | "tools" | "about";

type ClientTypeInfo = {
  name: string;
};

type SettingsDialogProps = {
  open: boolean;
  activeSection: SettingsSectionId;
  tauriRuntime: boolean;
  launcherState: LauncherState;
  clientPath: string;
  selectedClientType: ClientTypeInfo;
  customBgs: Record<string, { type: "default" | "image" | "video"; path: string }>;
  activeGameId: string;
  onCustomBgChange: (gameId: string, type: "default" | "image" | "video", path: string) => void;
  themeMode: "dark" | "light";
  onThemeChange: (theme: "dark" | "light") => void;
  errorMessage: string | null;
  settings: AppSettings;
  settingsState: "idle" | "loading" | "saving" | "saved" | "error";
  settingsError: string | null;
  smokeAutomation: LocalSmokeAutomationConfig | null;
  onClose: () => void;
  onSectionChange: (section: SettingsSectionId) => void;
  onUpdateSettings: (settings: AppSettings) => Promise<void>;
  onClientPathChange: (value: string) => void;
  onBrowse: () => Promise<void>;
  onValidate: () => Promise<void>;
};

const sections: { id: SettingsSectionId; label: string }[] = [
  { id: "general", label: "通用" },
  { id: "clients", label: "客户端" },
  { id: "download", label: "下载" },
  { id: "appearance", label: "外观" },
  { id: "tools", label: "工具" },
  { id: "about", label: "关于" }
];

function Toggle(props: { checked: boolean; label?: string; onChange: () => void }) {
  const buttonEl = (
    <button
      type="button"
      role="switch"
      aria-checked={props.checked}
      onClick={props.onChange}
      className={`w-11 h-6 rounded-full flex items-center transition-all duration-200 px-[3px] cursor-pointer border-[2.5px] bg-[#1f2229] ${
        props.checked
          ? "border-[#fed330]"
          : "border-[#4e525a]"
      }`}
    >
      <div
        className={`w-3.5 h-3.5 rounded-full transition-all duration-200 flex items-center justify-center shadow-sm ${
          props.checked
            ? "translate-x-5 bg-white text-[#111215]"
            : "translate-x-0 bg-[#c8c9cc] text-transparent"
        }`}
      >
        {props.checked && (
          <svg
            viewBox="0 0 24 24"
            className="w-2.5 h-2.5 stroke-[4.5]"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
        )}
      </div>
    </button>
  );

  if (!props.label) {
    return buttonEl;
  }

  return (
    <div className="flex items-center justify-between py-1 w-full">
      <span className="text-sm font-medium text-[var(--app-text-secondary)]">{props.label}</span>
      {buttonEl}
    </div>
  );
}

function SectionHeader(props: { children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
      <span className="text-sm font-bold text-[var(--app-text-muted)] uppercase tracking-wider">{props.children}</span>
    </div>
  );
}



export function SettingsDialog(props: SettingsDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const { onClose, open } = props;

  const [appVersion, setAppVersion] = useState<string>("0.1.0");
  const [updateStatus, setUpdateStatus] = useState<"idle" | "checking" | "up-to-date" | "has-update" | "failed">("idle");
  const [updateInfo, setUpdateInfo] = useState<AppUpdateCheck | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [showChangelog, setShowChangelog] = useState(false);
  const [appearanceGameId, setAppearanceGameId] = useState<string>(props.activeGameId);

  useEffect(() => {
    if (open) {
      setAppearanceGameId(props.activeGameId);
      setShowChangelog(false);
    }
  }, [open, props.activeGameId]);

  useEffect(() => {
    if (open && props.tauriRuntime) {
      void getAppVersion()
        .then((version) => setAppVersion(version))
        .catch((err) => console.error("Failed to get app version:", err));
    }
  }, [open, props.tauriRuntime]);

  useEffect(() => {
    setUpdateStatus("idle");
    setUpdateInfo(null);
    setUpdateError(null);
  }, [open, props.activeSection]);

  const handleCheckUpdate = () => {
    setUpdateStatus("checking");
    setUpdateError(null);
    checkAppUpdate()
      .then((res) => {
        setUpdateInfo(res);
        if (res.has_update) {
          setUpdateStatus("has-update");
        } else {
          setUpdateStatus("up-to-date");
        }
      })
      .catch((err) => {
        setUpdateStatus("failed");
        setUpdateError(err instanceof Error ? err.message : String(err));
      });
  };

  const handleSelectCustomBg = async (type: "image" | "video") => {
    if (!props.tauriRuntime) {
      const mockPath = type === "image"
        ? "https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1920&q=80"
        : "https://assets.mixkit.co/videos/preview/mixkit-abstract-laser-lights-background-32129-large.mp4";
      props.onCustomBgChange(appearanceGameId, type, mockPath);
      return;
    }

    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const selected = await openDialog({
        filters: type === "image"
          ? [{ name: "图片", extensions: ["jpg", "jpeg", "png", "webp", "gif", "svg"] }]
          : [{ name: "视频", extensions: ["mp4", "webm", "mkv", "avi", "mov"] }],
        multiple: false
      });
      if (selected && typeof selected === "string") {
        props.onCustomBgChange(appearanceGameId, type, selected);
      }
    } catch (err) {
      console.error("Failed to open custom background file:", err);
    }
  };

  const update = (settings: AppSettings) => {
    void props.onUpdateSettings(settings);
  };

  useEffect(() => {
    if (!open) return;

    const previousActiveElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    const firstFocusable = dialog?.querySelector<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    firstFocusable?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { onClose(); return; }
      if (event.key !== "Tab" || !dialog) return;

      const focusableElements = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'
        )
      );
      if (focusableElements.length === 0) { event.preventDefault(); return; }

      const first = focusableElements[0];
      const last = focusableElements[focusableElements.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousActiveElement?.focus();
    };
  }, [onClose, open]);

  const activeLabel = sections.find((s) => s.id === props.activeSection)?.label ?? "";

  const renderSection = () => {
    switch (props.activeSection) {
      case "general":
        return (
          <div className="space-y-6 text-sm text-[var(--app-text-secondary)]">
            {/* 启动设置 */}
            <div className="space-y-2">
              <div className="text-xs font-bold text-[#8f939c] uppercase tracking-wider pl-1">启动设置</div>
              <div className="bg-[#1f2229] border border-white/5 rounded-xl p-4 space-y-4">
                <div className="flex items-center justify-between py-1">
                  <div className="flex items-center space-x-2">
                    <span className="text-sm font-semibold text-white">开机自动运行 DDNet 启动器</span>
                    <span className="px-1.5 py-0.5 text-[10px] font-bold text-[#8f939c] bg-[#2d313d] rounded-[3px] scale-90 origin-left">推荐</span>
                  </div>
                  <Toggle
                    checked={props.settings.autostart}
                    onChange={() => update({ ...props.settings, autostart: !props.settings.autostart })}
                  />
                </div>
                <div className="border-t border-white/5" />
                <div className="flex items-center justify-between py-1">
                  <span className="text-sm font-semibold text-white">退出游戏后自动弹出启动器主界面</span>
                  <Toggle
                    checked={props.settings.exit_game_show_launcher}
                    onChange={() => update({ ...props.settings, exit_game_show_launcher: !props.settings.exit_game_show_launcher })}
                  />
                </div>
              </div>
            </div>

            {/* 关闭设置 */}
            <div className="space-y-2">
              <div className="text-xs font-bold text-[#8f939c] uppercase tracking-wider pl-1">关闭设置</div>
              <div className="bg-[#1f2229] border border-white/5 rounded-xl p-4 space-y-4">
                <div className="space-y-1">
                  <span className="text-sm font-semibold text-white block pb-1">关闭窗口</span>
                  <div className="space-y-3 pt-1">
                    {/* Radio 1: 最小化到系统托盘 */}
                    <button
                      type="button"
                      onClick={() => update({ ...props.settings, close_behavior: "minimize_to_tray" })}
                      className="flex items-center space-x-2.5 text-left cursor-pointer focus:outline-none"
                    >
                      <div className="w-4 h-4 rounded-full border border-[#8f939c] flex items-center justify-center relative">
                        {(props.settings.close_behavior === "minimize_to_tray" || props.settings.close_behavior === "ask") && (
                          <>
                            <div className="absolute inset-0 rounded-full border border-[#fed330]" />
                            <div className="w-2.5 h-2.5 rounded-full bg-[#fed330]" />
                          </>
                        )}
                      </div>
                      <span className="text-sm text-white/90 font-medium">最小化到系统托盘</span>
                      <span className="px-1.5 py-0.5 text-[10px] font-bold text-[#8f939c] bg-[#2d313d] rounded-[3px] scale-90 origin-left">推荐</span>
                    </button>

                    {/* Radio 2: 退出启动器 */}
                    <button
                      type="button"
                      onClick={() => update({ ...props.settings, close_behavior: "exit_launcher" })}
                      className="flex items-center space-x-2.5 text-left cursor-pointer focus:outline-none"
                    >
                      <div className="w-4 h-4 rounded-full border border-[#8f939c] flex items-center justify-center relative">
                        {props.settings.close_behavior === "exit_launcher" && (
                          <>
                            <div className="absolute inset-0 rounded-full border border-[#fed330]" />
                            <div className="w-2.5 h-2.5 rounded-full bg-[#fed330]" />
                          </>
                        )}
                      </div>
                      <span className="text-sm text-white/90 font-medium">退出启动器</span>
                    </button>
                  </div>
                </div>
              </div>
            </div>

            {/* 启动器更新 */}
            <div className="space-y-2">
              <div className="text-xs font-bold text-[#8f939c] uppercase tracking-wider pl-1">启动器更新</div>
              <div className="bg-[#1f2229] border border-white/5 rounded-xl p-4 space-y-2">
                <div className="flex items-center justify-between py-1">
                  <span className="text-sm font-semibold text-white">允许静默更新</span>
                  <Toggle
                    checked={props.settings.allow_silent_update}
                    onChange={() => update({ ...props.settings, allow_silent_update: !props.settings.allow_silent_update })}
                  />
                </div>
                <p className="text-xs text-[#8f939c] leading-relaxed max-w-[360px] pl-0.5">
                  允许 DDNet 启动器自动下载启动器更新包，完成后通知您进行安装
                </p>
              </div>
            </div>
          </div>
        );
      case "clients":
        return <ClientManager />;
      case "download":
        return (
          <UpdatePanel
            smokeAutomation={props.smokeAutomation}
            settings={props.settings}
            onUpdateSettings={props.onUpdateSettings}
          />
        );
      case "appearance": {
        const currentBg = props.customBgs[appearanceGameId] || { type: "default", path: "" };
        const appearanceGames = [
          { id: "qmclient", name: "QmClient" },
          { id: "ddnet", name: "DDNet" },
          { id: "ddnet-steam", name: "DDNet Steam" },
          { id: "third-party", name: "第三方客户端" }
        ];

        return (
          <div className="space-y-6 text-sm text-[var(--app-text-secondary)]">
            {/* 背景设置 */}
            <div className="space-y-3">
              <SectionHeader>背景</SectionHeader>
              
              {/* 客户端选择器 */}
              <div className="space-y-2 mt-2">
                <div className="text-xs font-bold text-[#8f939c] uppercase tracking-wider pl-1">当前定制客户端</div>
                <div className="flex bg-[#121316] p-1 rounded-xl border border-white/5 space-x-1">
                  {appearanceGames.map((g) => {
                    const isSel = appearanceGameId === g.id;
                    return (
                      <button
                        key={g.id}
                        type="button"
                        onClick={() => setAppearanceGameId(g.id)}
                        className={`flex-1 text-center py-2 rounded-lg text-xs font-bold transition-all cursor-pointer border border-transparent ${
                          isSel 
                            ? "bg-[#252932] text-[#fed330] border-white/5 shadow-md" 
                            : "text-white/60 hover:text-white hover:bg-white/5"
                        }`}
                      >
                        {g.name}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* 背景选项 */}
              <div className="bg-[#1f2229] border border-white/5 rounded-xl p-4 space-y-3">
                <div className="text-xs font-bold text-[#8f939c] uppercase tracking-wider pl-0.5">背景源</div>
                <div className="flex items-center space-x-2">
                  <button
                    type="button"
                    onClick={() => props.onCustomBgChange(appearanceGameId, "default", "")}
                    className={`px-4 py-2.5 rounded-lg text-xs font-extrabold tracking-wide transition-all cursor-pointer border ${
                      currentBg.type === "default"
                        ? "bg-[#2d313d] text-white border-white/10 shadow-md"
                        : "text-white/60 hover:text-white hover:bg-white/5 border-transparent"
                    }`}
                  >
                    默认背景
                  </button>
                  <button
                    type="button"
                    onClick={() => handleSelectCustomBg("image")}
                    className={`px-4 py-2.5 rounded-lg text-xs font-extrabold tracking-wide transition-all cursor-pointer border ${
                      currentBg.type === "image"
                        ? "bg-[#2d313d] text-white border-white/10 shadow-md"
                        : "text-white/60 hover:text-white hover:bg-white/5 border-transparent"
                    }`}
                  >
                    自定义图片
                  </button>
                  <button
                    type="button"
                    onClick={() => handleSelectCustomBg("video")}
                    className={`px-4 py-2.5 rounded-lg text-xs font-extrabold tracking-wide transition-all cursor-pointer border ${
                      currentBg.type === "video"
                        ? "bg-[#2d313d] text-white border-white/10 shadow-md"
                        : "text-white/60 hover:text-white hover:bg-white/5 border-transparent"
                    }`}
                  >
                    自定义视频
                  </button>
                </div>
                {currentBg.type !== "default" && currentBg.path && (
                  <div className="mt-2.5 p-3 bg-black/30 border border-white/5 rounded-lg flex items-center justify-between text-xs text-white/55 font-mono">
                    <span className="truncate max-w-[420px] select-text">
                      已配置{currentBg.type === "image" ? "图片" : "视频"}: {currentBg.path}
                    </span>
                    <button
                      type="button"
                      onClick={() => props.onCustomBgChange(appearanceGameId, "default", "")}
                      className="text-[#fed330] hover:text-[#ffe055] font-bold shrink-0 transition-colors ml-2 cursor-pointer border-none bg-transparent p-0"
                    >
                      还原默认
                    </button>
                  </div>
                )}
              </div>
            </div>

            {/* 主题设置 */}
            <div className="space-y-3">
              <SectionHeader>主题</SectionHeader>
              <div className="bg-[#1f2229] border border-white/5 rounded-xl p-4">
                <div className="flex items-center space-x-2">
                  <button
                    type="button"
                    onClick={() => props.onThemeChange("dark")}
                    className={`px-4 py-2.5 rounded-lg text-xs font-extrabold tracking-wide transition-all cursor-pointer border ${
                      props.themeMode === "dark"
                        ? "bg-[#2d313d] text-[#fed330] border-white/10 shadow-md"
                        : "text-white/60 hover:text-white hover:bg-white/5 border-transparent"
                    }`}
                  >
                    暗黑模式
                  </button>
                  <button
                    type="button"
                    onClick={() => props.onThemeChange("light")}
                    className={`px-4 py-2.5 rounded-lg text-xs font-extrabold tracking-wide transition-all cursor-pointer border ${
                      props.themeMode === "light"
                        ? "bg-[#2d313d] text-[#fed330] border-white/10 shadow-md"
                        : "text-white/60 hover:text-white hover:bg-white/5 border-transparent"
                    }`}
                  >
                    白天模式
                  </button>
                </div>
              </div>
            </div>
          </div>
        );
      }
      case "tools":
        return (
          <div className="space-y-5 text-sm text-[var(--app-text-secondary)]">
            <div className="space-y-3">
              <SectionHeader>扫描</SectionHeader>
              <div className="bg-[#1f2229] border border-white/5 rounded-xl p-4 space-y-3.5">
                <div className="space-y-1.5">
                  <span className="text-xs font-bold text-[var(--app-text-dim)] uppercase tracking-wider block">排除路径</span>
                  <textarea
                    aria-label="扫描排除路径列表"
                    value={props.settings.scan_excluded_paths.join("\n")}
                    onChange={(event) =>
                      update({
                        ...props.settings,
                        scan_excluded_paths: event.target.value
                          .split(/\r?\n/)
                          .flatMap((line) => { const t = line.trim(); return t ? [t] : []; })
                      })
                    }
                    placeholder="每行一个排除路径"
                    className="min-h-24 w-full resize-none bg-[var(--app-surface)] border border-white/5 rounded-lg px-3 py-2 text-sm text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] font-mono leading-normal transition-colors"
                  />
                </div>
              </div>
            </div>
          </div>
        );
      case "about":
        return (
          <div className="space-y-6 text-sm text-[var(--app-text-secondary)] leading-relaxed">
            <div className="flex items-center space-x-3.5 mb-6 mt-1">
              <img src={logoMark} alt="Logo" className="w-11 h-11 object-contain brightness-0 invert opacity-95" />
              <div className="flex items-center space-x-2.5 text-sm font-bold text-white">
                <span className="text-lg font-black tracking-wide">DDNet Manager</span>
                <span className="text-white/30 font-normal">|</span>
                <span className="text-white/45 font-mono font-medium">V{appVersion}</span>
              </div>
            </div>

            <div className="bg-[#1f2229] border border-white/5 rounded-xl overflow-hidden shadow-md">
              {/* Item 1: 查看更新日志 */}
              <button
                type="button"
                onClick={() => setShowChangelog(!showChangelog)}
                className="w-full flex items-center justify-between px-5 py-4 text-left hover:bg-white/5 transition-colors cursor-pointer focus:outline-none"
              >
                <span className="text-sm font-bold text-white/90">查看更新日志</span>
                <svg viewBox="0 0 24 24" className={`w-4 h-4 text-white/35 fill-none stroke-current stroke-2 transition-transform duration-200 ${showChangelog ? "rotate-90" : ""}`}>
                  <polyline points="9 18 15 12 9 6" />
                </svg>
              </button>

              <div className="h-[1px] bg-white/5 mx-5" />

              {/* Item 2: 检查最新版本 */}
              <div className="flex items-center justify-between px-5 py-3.5">
                <span className="text-sm font-bold text-white/90">检查最新版本</span>
                {updateStatus === "checking" ? (
                  <div className="flex items-center space-x-2 text-white/40 text-xs font-bold">
                    <Loader2 className="w-3.5 h-3.5 animate-spin text-[#fed330]" />
                    <span>正在检查...</span>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={handleCheckUpdate}
                    className="px-3 py-1.5 rounded-lg text-xs font-bold border border-white/15 hover:border-white/35 text-white/75 hover:text-white bg-transparent hover:bg-white/5 flex items-center space-x-1.5 transition-all cursor-pointer focus:outline-none"
                  >
                    <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 fill-none stroke-current stroke-[2.5]">
                      <circle cx="11" cy="11" r="8" />
                      <line x1="21" y1="21" x2="16.65" y2="16.65" />
                    </svg>
                    <span>检查</span>
                  </button>
                )}
              </div>
            </div>

            <AnimatePresence>
              {showChangelog && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                  className="overflow-hidden"
                >
                  <div className="p-4 bg-black/25 border border-white/5 rounded-xl space-y-2.5">
                    <div className="text-xs font-black text-[#8f939c] uppercase tracking-wider">更新日志历史</div>
                    <div className="text-xs font-mono text-white/70 space-y-2 leading-relaxed max-h-40 overflow-y-auto pr-1">
                      <p className="font-bold text-white">v0.1.2 (当前版本)</p>
                      <ul className="list-disc list-inside pl-1 space-y-1 text-white/60">
                        <li>优化设置页面，字体大小全面提升，阅读更清晰。</li>
                        <li>重构“关于”页面布局，完美还原米哈游样式。</li>
                        <li>“外观”设置全新设计，支持各客户端独立定制背景（图片/视频）。</li>
                        <li>外观新增白天与暗黑主题模式预设。</li>
                      </ul>
                      <p className="font-bold text-white mt-3.5">v0.1.1</p>
                      <ul className="list-disc list-inside pl-1 space-y-1 text-white/60">
                        <li>新增下载与更新功能面板。</li>
                        <li>接入 SQLite 本地历史记录与断点下载恢复。</li>
                      </ul>
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {updateStatus === "up-to-date" && (
              <div className="flex items-center space-x-2.5 p-3 bg-emerald-950/20 border border-emerald-500/20 rounded-xl text-emerald-400 text-xs">
                <CheckCircle2 className="w-4 h-4" />
                <span className="font-semibold">当前已是最新版本，无需更新！</span>
              </div>
            )}

            {updateStatus === "has-update" && updateInfo && (
              <div className="space-y-3 p-4 bg-amber-950/20 border border-amber-500/20 rounded-xl text-xs">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2.5 text-amber-400">
                    <div className="w-2 h-2 rounded-full bg-amber-400 animate-pulse" />
                    <span className="font-semibold text-sm">发现新版本 v{updateInfo.latest_version}</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => window.open(updateInfo.release_url, "_blank", "noreferrer")}
                    className="flex items-center space-x-1 px-3.5 py-1.5 rounded-lg font-bold bg-amber-500 text-black hover:bg-amber-400 transition-all cursor-pointer focus:outline-none"
                  >
                    <span>前往下载</span>
                    <ArrowUpRight className="w-3.5 h-3.5" />
                  </button>
                </div>
                {updateInfo.release_notes && (
                  <div className="space-y-1.5">
                    <span className="text-[10px] font-bold text-[var(--app-text-dim)] uppercase tracking-wider block">版本更新详情</span>
                    <div className="p-3 bg-black/40 border border-white/5 rounded-lg text-xs font-mono text-[var(--app-text-secondary)] max-h-36 overflow-y-auto whitespace-pre-wrap select-text leading-relaxed">
                      {updateInfo.release_notes}
                    </div>
                  </div>
                )}
              </div>
            )}

            {updateStatus === "failed" && (
              <div className="flex items-start space-x-2.5 p-3 bg-red-950/20 border border-red-500/20 rounded-xl text-red-400 text-xs">
                <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                <div className="flex-1">
                  <span className="font-semibold block">检查更新失败</span>
                  <span className="text-[10px] text-red-300/80 block mt-0.5 leading-normal">{updateError}</span>
                </div>
              </div>
            )}
          </div>
        );
    }
  };

  return (
    <AnimatePresence>
      {props.open ? (
        <motion.div
          className="absolute inset-0 flex items-center justify-center z-50 select-none"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) props.onClose();
          }}
        >
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="absolute inset-0 bg-black/35 backdrop-blur-[2px]"
          />
          <motion.section
            ref={dialogRef}
            role="dialog"
            aria-modal="true"
            aria-label="设置"
            initial={{ opacity: 0, scale: 0.95, y: 15 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 15 }}
            transition={{ duration: 0.22, ease: "easeOut" }}
            className="relative z-10 w-[720px] h-[540px] bg-[var(--app-surface)] border border-[var(--app-border)] rounded-2xl shadow-[0_12px_40px_rgba(0,0,0,0.6)] overflow-hidden flex flex-col font-sans text-[var(--app-text-secondary)]"
          >
            <div className="flex-1 flex overflow-hidden">
              {/* Left sidebar */}
              <div className="w-[180px] bg-[var(--app-surface-alt)] border-r border-[var(--app-border-subtle)] p-4 flex flex-col justify-between">
                <div className="space-y-1.5">
                  <span className="text-[var(--app-text)] text-base font-bold pl-2.5 pb-2.5 block tracking-wide">设置</span>
                  {sections.map((section) => {
                    const isActive = props.activeSection === section.id;
                    return (
                      <button
                        key={section.id}
                        type="button"
                        onClick={() => props.onSectionChange(section.id)}
                        className={`w-full text-left px-3.5 py-2 rounded-lg text-xs font-semibold tracking-wide transition-all cursor-pointer border ${
                          isActive
                            ? "bg-[var(--app-border-strong)] text-[var(--app-text)] shadow-sm border-[var(--app-border-subtle)]"
                            : "text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] hover:bg-[var(--app-border-subtle)] border-transparent"
                        }`}
                      >
                        {section.label}
                      </button>
                    );
                  })}
                </div>
                <div className="pl-2.5 text-[10px] text-[var(--app-text-dim)] font-mono">
                  {props.settingsState === "saving" ? "保存中..." : props.settingsState === "error" ? "保存失败" : ""}
                </div>
              </div>

              {/* Right content */}
              <div className="flex-1 bg-[var(--app-surface)] p-6 overflow-y-auto relative">
                <button
                  type="button"
                  aria-label="关闭设置"
                  onClick={props.onClose}
                  className="absolute top-4 right-4 w-7 h-7 rounded-full bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] flex items-center justify-center text-[var(--app-text-muted)] hover:text-[var(--app-text)] transition-colors cursor-pointer z-10"
                >
                  <X className="w-4 h-4" />
                </button>

                <h2 className="text-base font-bold text-[var(--app-text)] mb-6 tracking-wide">{activeLabel}</h2>
                {renderSection()}
              </div>
            </div>
          </motion.section>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
