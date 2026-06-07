import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useRef } from "react";
import { GameIcon, type GameIconName } from "@/components/icons/GameIcon";
import { Button } from "@/components/ui/button";
import type { ClientType } from "@/components/games/GamesPanel";
import type { AppSettings, LauncherState, NetworkRouteMode } from "@/types";

export type SettingsSectionId = "general" | "download" | "appearance" | "tools" | "about";

type SettingsSection = {
  id: SettingsSectionId;
  label: string;
  icon: GameIconName;
};

type SettingsDialogProps = {
  open: boolean;
  activeSection: SettingsSectionId;
  launcherState: LauncherState;
  clientPath: string;
  selectedClientType: ClientType;
  backgroundMode: "default" | "custom";
  errorMessage: string | null;
  settings: AppSettings;
  settingsState: "idle" | "loading" | "saving" | "saved" | "error";
  settingsError: string | null;
  onClose: () => void;
  onSectionChange: (section: SettingsSectionId) => void;
  onSettingsChange: (settings: AppSettings) => void;
  onSaveSettings: () => Promise<void>;
  onClientPathChange: (value: string) => void;
  onBrowse: () => Promise<void>;
  onValidate: () => Promise<void>;
  onBackgroundImageSelect: (file: File) => Promise<void>;
  onClearBackgroundImage: () => void;
};

const sections: SettingsSection[] = [
  { id: "general", label: "通用", icon: "settings" },
  { id: "download", label: "下载", icon: "cloudDownload" },
  { id: "appearance", label: "外观", icon: "folder" },
  { id: "tools", label: "工具", icon: "wrench" },
  { id: "about", label: "关于", icon: "gamepad" }
];

function SettingCard(props: { title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-[24px] bg-[var(--dm-soft)] p-4">
      <h3 className="text-base font-black text-[var(--dm-ink)]">{props.title}</h3>
      <div className="mt-3">{props.children}</div>
    </section>
  );
}

function TogglePill(props: { checked: boolean; label: string }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-[22px] bg-white/70 px-4 py-3">
      <span className="text-sm font-bold text-[var(--dm-ink)]">{props.label}</span>
      <span
        className={`relative h-7 w-12 rounded-full border transition ${
          props.checked ? "border-[var(--dm-ink)] bg-[var(--dm-ink)]" : "border-[var(--dm-border)] bg-white"
        }`}
      >
        <span
          className={`absolute top-1 grid h-5 w-5 place-items-center rounded-full bg-white shadow-sm transition ${
            props.checked ? "left-6" : "left-1"
          }`}
        >
          {props.checked ? <GameIcon name="play" className="size-3 text-[var(--dm-ink)]" /> : null}
        </span>
      </span>
    </div>
  );
}

function routeHostFromUrl(value: string) {
  try {
    return new URL(value).hostname;
  } catch {
    return "";
  }
}

function networkRouteUrl(settings: AppSettings) {
  return settings.network_route?.proxy_prefix_url ?? settings.network_route?.mirror_template ?? "";
}

function updateNetworkRoute(settings: AppSettings, mode: NetworkRouteMode, rawUrl: string): AppSettings {
  const trimmedUrl = rawUrl.trim();
  if (mode === "direct") {
    return { ...settings, network_route: null };
  }

  const host = routeHostFromUrl(trimmedUrl);
  return {
    ...settings,
    network_route: {
      mode,
      proxy_prefix_url: mode === "proxy_prefix" ? trimmedUrl : null,
      mirror_template: mode === "mirror_template" ? trimmedUrl : null,
      enabled_hosts: host ? [host] : []
    }
  };
}

export function SettingsDialog(props: SettingsDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const { onClose, open } = props;
  const canValidate = props.clientPath.trim().length > 0 && props.launcherState !== "validating" && props.launcherState !== "launching";

  useEffect(() => {
    if (!open) {
      return;
    }

    const previousActiveElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    const firstFocusable = dialog?.querySelector<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );

    firstFocusable?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
        return;
      }

      if (event.key !== "Tab" || !dialog) {
        return;
      }

      const focusableElements = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'
        )
      );

      if (focusableElements.length === 0) {
        event.preventDefault();
        return;
      }

      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];

      if (event.shiftKey && document.activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && document.activeElement === lastElement) {
        event.preventDefault();
        firstElement.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousActiveElement?.focus();
    };
  }, [onClose, open]);

  const renderSection = () => {
    switch (props.activeSection) {
      case "general":
        return (
          <div className="space-y-4">
            <SettingCard title="客户端">
              <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto_auto]">
                <input
                  value={props.clientPath}
                  onChange={(event) => props.onClientPathChange(event.target.value)}
                  placeholder="C:/Games/QmClient"
                  className="h-12 min-w-0 rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#9a9fa8] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
                />
                <button
                  type="button"
                  onClick={() => void props.onBrowse()}
                  className="h-12 rounded-[18px] border border-[var(--dm-border)] bg-white px-5 text-sm font-black text-[var(--dm-ink)] transition hover:-translate-y-0.5 hover:bg-[var(--dm-paper)]"
                >
                  定位
                </button>
                <button
                  type="button"
                  onClick={() => void props.onValidate()}
                  disabled={!canValidate}
                  className="h-12 rounded-[18px] bg-[var(--dm-ink)] px-5 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
                >
                  验证
                </button>
              </div>
              {props.errorMessage ? (
                <div className="mt-3 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">
                  {props.errorMessage}
                </div>
              ) : null}
            </SettingCard>
            <SettingCard title="启动">
              <div className="grid gap-3 md:grid-cols-2">
                <TogglePill checked label="启动后关闭面板" />
                <TogglePill checked={false} label="自动检查更新" />
              </div>
            </SettingCard>
          </div>
        );
      case "download":
        return (
          <div className="space-y-4">
            <SettingCard title="网络">
              <div className="grid gap-3">
                <div className="flex flex-wrap gap-2">
                  {(["direct", "proxy_prefix", "mirror_template"] as const).map((mode) => {
                    const active = (props.settings.network_route?.mode ?? "direct") === mode;
                    return (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => props.onSettingsChange(updateNetworkRoute(props.settings, mode, networkRouteUrl(props.settings)))}
                        className={`h-10 rounded-[15px] border px-4 text-xs font-black transition hover:-translate-y-0.5 ${
                          active
                            ? "border-[var(--dm-ink)] bg-[var(--dm-ink)] text-white"
                            : "border-[var(--dm-border)] bg-white text-[var(--dm-muted-ink)]"
                        }`}
                      >
                        {mode === "direct" ? "直连" : mode === "proxy_prefix" ? "代理前缀" : "镜像模板"}
                      </button>
                    );
                  })}
                </div>
                {(props.settings.network_route?.mode ?? "direct") !== "direct" ? (
                  <input
                    value={networkRouteUrl(props.settings)}
                    onChange={(event) =>
                      props.onSettingsChange(
                        updateNetworkRoute(
                          props.settings,
                          props.settings.network_route?.mode ?? "proxy_prefix",
                          event.target.value
                        )
                      )
                    }
                    placeholder={props.settings.network_route?.mode === "mirror_template" ? "https://mirror.example/{url}" : "https://proxy.example/"}
                    className="h-12 min-w-0 rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#9a9fa8] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
                  />
                ) : null}
              </div>
            </SettingCard>
            <SettingCard title="高级更新源">
              <input
                value={props.settings.advanced_manifest_url ?? ""}
                onChange={(event) =>
                  props.onSettingsChange({
                    ...props.settings,
                    advanced_manifest_url: event.target.value.trim() ? event.target.value : null
                  })
                }
                placeholder="https://gitee.com/example/manifest/raw/main/ddnet.json"
                className="h-12 w-full rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#9a9fa8] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
              />
            </SettingCard>
            <SettingCard title="GitHub">
              <input
                value={props.settings.github_token ?? ""}
                onChange={(event) =>
                  props.onSettingsChange({
                    ...props.settings,
                    github_token: event.target.value.trim() ? event.target.value : null
                  })
                }
                placeholder="GitHub token"
                type="password"
                className="h-12 w-full rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#9a9fa8] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
              />
            </SettingCard>
          </div>
        );
      case "appearance":
        return (
          <div className="space-y-4">
            <SettingCard title="背景">
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={props.onClearBackgroundImage}
                  className={`h-11 rounded-[16px] border px-4 text-sm font-black transition hover:-translate-y-0.5 ${
                    props.backgroundMode === "default"
                      ? "border-[var(--dm-ink)] bg-[var(--dm-ink)] text-white"
                      : "border-[var(--dm-border)] bg-white text-[var(--dm-muted-ink)]"
                  }`}
                >
                  默认背景
                </button>
                <label className="grid h-11 cursor-pointer place-items-center rounded-[16px] border border-[var(--dm-border)] bg-white px-4 text-sm font-black text-[var(--dm-muted-ink)] transition hover:-translate-y-0.5 hover:bg-[var(--dm-paper)]">
                  自定义图片
                  <input
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={(event) => {
                      const file = event.target.files?.[0];
                      event.currentTarget.value = "";
                      if (file) {
                        void props.onBackgroundImageSelect(file);
                      }
                    }}
                  />
                </label>
              </div>
            </SettingCard>
            <SettingCard title="主题">
              <div className="rounded-[22px] bg-white/70 px-4 py-3 text-sm font-bold text-[var(--dm-muted-ink)]">
                白天模式
              </div>
            </SettingCard>
          </div>
        );
      case "tools":
        return (
          <div className="space-y-4">
            <SettingCard title="扫描">
              <div className="grid gap-3">
                <button
                  type="button"
                  onClick={() => props.onSettingsChange({ ...props.settings, use_everything: !props.settings.use_everything })}
                  className="text-left"
                >
                  <TogglePill checked={props.settings.use_everything} label="使用 Everything 加速扫描" />
                </button>
                <textarea
                  value={props.settings.scan_excluded_paths.join("\n")}
                  onChange={(event) =>
                    props.onSettingsChange({
                      ...props.settings,
                      scan_excluded_paths: event.target.value
                        .split(/\r?\n/)
                        .map((line) => line.trim())
                        .filter(Boolean)
                    })
                  }
                  placeholder="每行一个排除路径"
                  className="min-h-28 w-full resize-none rounded-[18px] border border-[var(--dm-border)] bg-white px-4 py-3 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#9a9fa8] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
                />
              </div>
            </SettingCard>
          </div>
        );
      case "about":
        return (
          <div className="rounded-[28px] bg-[var(--dm-soft)] p-5">
            <div className="flex items-center gap-4">
              <div className="grid h-14 w-14 place-items-center rounded-2xl bg-white text-[var(--dm-ink)]">
                <GameIcon name={props.selectedClientType.icon} className="size-8" />
              </div>
              <div>
                <div className="text-2xl font-black tracking-[-0.04em] text-[var(--dm-ink)]">DDNet Manager</div>
                <div className="mt-1 text-sm font-bold text-[var(--dm-muted-ink)]">{props.selectedClientType.name}</div>
              </div>
            </div>
          </div>
        );
    }
  };

  return (
    <AnimatePresence>
      {props.open ? (
        <motion.div
          className="fixed inset-0 z-[80] grid place-items-center bg-[#2f3440]/24 p-6 backdrop-blur-sm"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              props.onClose();
            }
          }}
        >
          <motion.section
            ref={dialogRef}
            role="dialog"
            aria-modal="true"
            aria-label="设置"
            initial={{ opacity: 0, y: 18, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 12, scale: 0.98 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="grid h-[min(78vh,720px)] w-[min(92vw,1120px)] overflow-hidden rounded-[30px] border border-[var(--dm-border)] bg-[var(--dm-paper)] text-[var(--dm-ink)] shadow-[0_34px_110px_rgba(47,52,64,0.22)] md:grid-cols-[300px_minmax(0,1fr)]"
          >
            <aside className="border-r border-[var(--dm-border)] bg-[var(--dm-panel)] p-6">
              <div className="text-3xl font-black tracking-[-0.05em]">设置</div>
              <div className="mt-8 grid gap-2">
                {sections.map((section) => {
                  const active = props.activeSection === section.id;
                  return (
                    <button
                      key={section.id}
                      type="button"
                      onClick={() => props.onSectionChange(section.id)}
                      className={`flex h-14 items-center gap-3 rounded-2xl px-4 text-left text-base font-black transition ${
                        active
                          ? "bg-[var(--dm-ink)] text-white shadow-[0_14px_30px_rgba(47,52,64,0.16)]"
                          : "text-[var(--dm-muted-ink)] hover:bg-white/68 hover:text-[var(--dm-ink)]"
                      }`}
                    >
                      <GameIcon name={section.icon} className="size-5" />
                      {section.label}
                    </button>
                  );
                })}
              </div>
            </aside>
            <div className="dm-scroll min-h-0 overflow-y-auto p-6">
              <div className="mb-7 flex items-center justify-between gap-4">
                <h2 className="text-3xl font-black tracking-[-0.05em]">
                  {sections.find((section) => section.id === props.activeSection)?.label}
                </h2>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label="关闭设置"
                  onClick={props.onClose}
                  className="h-11 w-11 rounded-2xl text-[var(--dm-muted-ink)] hover:bg-[var(--dm-soft)] hover:text-[var(--dm-ink)]"
                >
                  ×
                </Button>
              </div>
              <div className="mb-4 flex items-center justify-between gap-3 rounded-[18px] bg-[var(--dm-soft)] px-4 py-3">
                <div className="text-xs font-bold text-[var(--dm-muted-ink)]">
                  {props.settingsError ?? (props.settingsState === "saved" ? "设置已保存" : "设置变更会写入本机注册表")}
                </div>
                <button
                  type="button"
                  onClick={() => void props.onSaveSettings()}
                  disabled={props.settingsState === "saving" || props.settingsState === "loading"}
                  className="h-9 rounded-[14px] bg-[var(--dm-ink)] px-4 text-xs font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
                >
                  {props.settingsState === "saving" ? "保存中" : "保存"}
                </button>
              </div>
              {renderSection()}
            </div>
          </motion.section>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
