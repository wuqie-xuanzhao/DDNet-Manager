import { AnimatePresence, motion } from "framer-motion";
import { X, Loader2, ArrowUpRight, CheckCircle2, AlertCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ClientManager } from "@/components/clients/ClientManager";
import { UpdatePanel } from "@/components/update/UpdatePanel";
import { networkRouteUrl, updateNetworkRoute } from "@/lib/settings";
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
  backgroundMode: "default" | "custom";
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
  onBackgroundImageSelect: (file: File) => Promise<void>;
  onClearBackgroundImage: () => void;
};

const sections: { id: SettingsSectionId; label: string }[] = [
  { id: "general", label: "通用" },
  { id: "clients", label: "客户端" },
  { id: "download", label: "下载" },
  { id: "updates", label: "更新" },
  { id: "appearance", label: "外观" },
  { id: "tools", label: "工具" },
  { id: "about", label: "关于" }
];

function Toggle(props: { checked: boolean; label: string; onChange: () => void }) {
  return (
    <div className="flex items-center justify-between py-1">
      <span className="text-xs font-medium text-[var(--app-text-secondary)]">{props.label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={props.checked}
        onClick={props.onChange}
        className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
          props.checked ? "bg-[var(--app-accent)]" : "bg-[var(--app-input)]"
        }`}
      >
        <div
          className={`w-[14px] h-[14px] rounded-full transition-transform bg-white shadow-sm ${
            props.checked ? "translate-x-5" : "translate-x-0"
          }`}
        />
      </button>
    </div>
  );
}

function SectionHeader(props: { children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
      <span className="text-[var(--app-text-muted)] text-xs font-bold uppercase tracking-wider">{props.children}</span>
    </div>
  );
}

function InputField(props: { value: string; onChange: (value: string) => void; placeholder?: string; type?: string; "aria-label"?: string }) {
  return (
    <input
      type={props.type ?? "text"}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
      placeholder={props.placeholder}
      aria-label={props["aria-label"]}
      className="bg-[var(--app-surface)] border border-[var(--app-border-subtle)] rounded-lg px-3.5 py-2 w-full text-xs text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] font-mono transition-colors"
    />
  );
}

export function SettingsDialog(props: SettingsDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const { onClose, open } = props;

  const [appVersion, setAppVersion] = useState<string>("0.1.0");
  const [updateStatus, setUpdateStatus] = useState<"idle" | "checking" | "up-to-date" | "has-update" | "failed">("idle");
  const [updateInfo, setUpdateInfo] = useState<AppUpdateCheck | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

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
          <div className="space-y-6">
            <div className="space-y-3">
              <SectionHeader>启动选项</SectionHeader>
              <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 space-y-3.5">
                <Toggle
                  checked={props.settings.close_panel_after_launch}
                  label="启动后最小化启动器"
                  onChange={() => update({ ...props.settings, close_panel_after_launch: !props.settings.close_panel_after_launch })}
                />
                <div className="border-t border-[var(--app-border-subtle)]" />
                <Toggle
                  checked={props.settings.auto_check_updates}
                  label="自动检查更新"
                  onChange={() => update({ ...props.settings, auto_check_updates: !props.settings.auto_check_updates })}
                />
              </div>
            </div>
          </div>
        );
      case "clients":
        return <ClientManager />;
      case "download":
        return (
          <div className="space-y-5">
            <div className="space-y-3">
              <SectionHeader>网络路由</SectionHeader>
              <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 space-y-3">
                <div className="flex flex-wrap gap-2">
                  {(["direct", "proxy_prefix", "mirror_template"] as const).map((mode) => {
                    const active = (props.settings.network_route?.mode ?? "direct") === mode;
                    return (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => update(updateNetworkRoute(props.settings, mode, networkRouteUrl(props.settings)))}
                        className={`px-3 py-1.5 rounded-lg text-xs font-semibold tracking-wide transition-all cursor-pointer ${
                          active
                            ? "bg-[var(--app-border-strong)] text-[var(--app-text)] shadow-sm border border-[var(--app-border-subtle)]"
                            : "text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] hover:bg-black/20"
                        }`}
                      >
                        {mode === "direct" ? "直连" : mode === "proxy_prefix" ? "代理前缀" : "镜像模板"}
                      </button>
                    );
                  })}
                </div>
                {(props.settings.network_route?.mode ?? "direct") !== "direct" ? (
                  <InputField
                    aria-label={props.settings.network_route?.mode === "mirror_template" ? "镜像模板地址" : "代理前缀地址"}
                    value={networkRouteUrl(props.settings)}
                    onChange={(value) => update(updateNetworkRoute(props.settings, props.settings.network_route?.mode ?? "proxy_prefix", value))}
                    placeholder={props.settings.network_route?.mode === "mirror_template" ? "https://mirror.example/{url}" : "https://proxy.example/"}
                  />
                ) : null}
              </div>
            </div>
            <div className="space-y-3">
              <SectionHeader>高级更新源</SectionHeader>
              <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4">
                <InputField
                  aria-label="manifest 地址"
                  value={props.settings.advanced_manifest_url ?? ""}
                  onChange={(value) => update({ ...props.settings, advanced_manifest_url: value.trim() ? value : null })}
                  placeholder="https://gitee.com/example/manifest/raw/main/ddnet.json"
                />
              </div>
            </div>
          </div>
        );
      case "updates":
        return <UpdatePanel smokeAutomation={props.smokeAutomation} />;
      case "appearance":
        return (
          <div className="space-y-5">
            <div className="space-y-3">
              <SectionHeader>背景</SectionHeader>
              <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4">
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={props.onClearBackgroundImage}
                    className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold tracking-wide transition-all cursor-pointer ${
                      props.backgroundMode === "default"
                        ? "bg-[var(--app-border-strong)] text-[var(--app-text)] shadow-sm border border-[var(--app-border-subtle)]"
                        : "text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] hover:bg-black/20"
                    }`}
                  >
                    默认背景
                  </button>
                  <label className="px-3.5 py-1.5 rounded-lg text-xs font-semibold tracking-wide text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] hover:bg-black/20 cursor-pointer transition-all border border-transparent hover:border-[var(--app-border-subtle)]">
                    自定义图片
                    <input
                      type="file"
                      accept="image/*"
                      className="hidden"
                      onChange={(event) => {
                        const file = event.target.files?.[0];
                        event.currentTarget.value = "";
                        if (file) void props.onBackgroundImageSelect(file);
                      }}
                    />
                  </label>
                </div>
              </div>
            </div>
            <div className="space-y-3">
              <SectionHeader>主题</SectionHeader>
              <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 text-xs text-[var(--app-text-muted)] font-medium">
                暗黑
              </div>
            </div>
          </div>
        );
      case "tools":
        return (
          <div className="space-y-5">
            <div className="space-y-3">
              <SectionHeader>扫描</SectionHeader>
              <div className="bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl p-4 space-y-3.5">
                <Toggle
                  checked={props.settings.use_everything}
                  label="使用 Everything 加速扫描"
                  onChange={() => update({ ...props.settings, use_everything: !props.settings.use_everything })}
                />
                <div className="border-t border-[var(--app-border-subtle)]" />
                <div className="space-y-1.5">
                  <span className="text-[10px] font-bold text-[var(--app-text-dim)] uppercase tracking-wider block">排除路径</span>
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
                    className="min-h-24 w-full resize-none bg-[var(--app-surface)] border border-[var(--app-border-subtle)] rounded-lg px-3 py-2 text-xs text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] font-mono leading-normal transition-colors"
                  />
                </div>
              </div>
            </div>
          </div>
        );
      case "about":
        return (
          <div className="space-y-5 text-xs text-[var(--app-text-muted)] leading-relaxed">
            <SectionHeader>关于</SectionHeader>
            <div className="flex items-center space-x-4 p-2 bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl">
              <img src={logoMark} alt="Logo" className="w-12 h-12 object-contain" />
              <div className="flex-1">
                <span className="text-sm font-bold text-[var(--app-text)] block tracking-wide">DDNet Manager</span>
                <div className="flex items-center space-x-2 mt-1">
                  <span className="text-[var(--app-text-secondary)] font-semibold">版本 v{appVersion}</span>
                  <span className="text-[10px] bg-[var(--app-border-strong)] text-[var(--app-text-dim)] px-2 py-0.5 rounded-full font-mono">
                    {props.selectedClientType.name}
                  </span>
                </div>
              </div>
              <div>
                {updateStatus !== "checking" && (
                  <button
                    type="button"
                    onClick={handleCheckUpdate}
                    className="px-3.5 py-1.5 rounded-lg text-xs font-semibold tracking-wide bg-[var(--app-accent)] text-black hover:bg-cyan-400 hover:shadow-[0_0_12px_rgba(65,242,255,0.4)] transition-all cursor-pointer"
                  >
                    检查更新
                  </button>
                )}
              </div>
            </div>

            {updateStatus === "checking" && (
              <div className="flex items-center space-x-2.5 p-3 bg-[var(--app-input)] border border-[var(--app-border-subtle)] rounded-xl text-[var(--app-text-secondary)] animate-pulse">
                <Loader2 className="w-4 h-4 animate-spin text-[var(--app-accent)]" />
                <span className="font-medium">正在获取最新版本信息...</span>
              </div>
            )}

            {updateStatus === "up-to-date" && (
              <div className="flex items-center space-x-2.5 p-3 bg-emerald-950/20 border border-emerald-500/30 rounded-xl text-emerald-400">
                <CheckCircle2 className="w-4 h-4" />
                <span className="font-semibold">当前已是最新版本，无需更新！</span>
              </div>
            )}

            {updateStatus === "has-update" && updateInfo && (
              <div className="space-y-3 p-4 bg-amber-950/20 border border-amber-500/20 rounded-xl">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2.5 text-amber-400">
                    <div className="w-2 h-2 rounded-full bg-amber-400 animate-pulse" />
                    <span className="font-semibold text-sm">发现新版本 v{updateInfo.latest_version}</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => window.open(updateInfo.release_url, "_blank", "noreferrer")}
                    className="flex items-center space-x-1 px-3 py-1.5 rounded-lg text-xs font-bold bg-amber-500 text-black hover:bg-amber-400 transition-all cursor-pointer shadow-md shadow-amber-500/10"
                  >
                    <span>前往下载</span>
                    <ArrowUpRight className="w-3.5 h-3.5" />
                  </button>
                </div>

                {updateInfo.release_notes && (
                  <div className="space-y-1.5">
                    <span className="text-[10px] font-bold text-[var(--app-text-dim)] uppercase tracking-wider block">更新日志</span>
                    <div className="p-3 bg-black/40 border border-[var(--app-border-subtle)] rounded-lg text-xs font-mono text-[var(--app-text-secondary)] max-h-36 overflow-y-auto whitespace-pre-wrap select-text leading-relaxed">
                      {updateInfo.release_notes}
                    </div>
                  </div>
                )}
              </div>
            )}

            {updateStatus === "failed" && (
              <div className="flex items-start space-x-2.5 p-3 bg-red-950/20 border border-red-500/20 rounded-xl text-red-400">
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
            className="absolute inset-0 bg-black/70 backdrop-blur-sm"
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
                        className={`w-full text-left px-3.5 py-2 rounded-lg text-xs font-semibold tracking-wide transition-all cursor-pointer ${
                          isActive
                            ? "bg-[var(--app-border-strong)] text-[var(--app-text)] shadow-sm border border-[var(--app-border-subtle)]"
                            : "text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] hover:bg-[var(--app-border-subtle)]"
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
