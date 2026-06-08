import { startTransition, useEffect, useState, type ReactNode } from "react";
import { motion } from "framer-motion";
import { Bell, CircleHelp, Disc3, Headphones, MessageCircle, Radio, Share2 } from "lucide-react";
import launcherBackground from "./assets/launcher-background.png";
import logoMark from "./assets/logo.svg";
import { ClientManager } from "./components/clients/ClientManager";
import { GamesPanel, type ClientType } from "./components/games/GamesPanel";
import { GameIcon, type GameIconName } from "./components/icons/GameIcon";
import { TitleBar } from "./components/layout/TitleBar";
import { LaunchPanel } from "./components/launch/LaunchPanel";
import { SettingsDialog, type SettingsSectionId } from "./components/settings/SettingsDialog";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { UpdatePanel } from "./components/update/UpdatePanel";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAutoUpdate, type AutoUpdateState } from "./hooks/useAutoUpdate";
import { useClientLauncher } from "./hooks/useClientLauncher";
import { isTauriRuntime } from "./lib/tauri";
import type {
  ClientInstallation,
  ClientUpdateCheck,
  LocalSmokeAutomationConfig
} from "./types";

type AppView = "launch" | "games" | "update";
type BackgroundMode = "default" | "custom";

type NavItem = {
  id: AppView;
  label: string;
  icon: GameIconName;
};

const navItems: NavItem[] = [
  { id: "launch", label: "启动", icon: "play" },
  { id: "update", label: "更新", icon: "cloudDownload" }
];

const clientTypes: ClientType[] = [
  {
    id: "qmclient",
    name: "QmClient",
    subtitle: "主力客户端",
    version: "稳定版",
    status: "已选",
    pathHint: "等待定位",
    icon: "gamepad",
    accent: "#2f3440"
  },
  {
    id: "ddnet",
    name: "DDNet Vanilla",
    subtitle: "官网下载",
    version: "官方版",
    status: "可添加",
    pathHint: "未定位",
    icon: "play",
    accent: "#6d7280"
  },
  {
    id: "ddnet-steam",
    name: "DDNet Steam",
    subtitle: "Steam 安装",
    version: "Steam",
    status: "可扫描",
    pathHint: "steamapps/common/DDNet",
    icon: "folder",
    accent: "#4e657a"
  },
  {
    id: "qmclient-nightly",
    name: "QmClient Nightly",
    subtitle: "测试通道",
    version: "测试版",
    status: "暂未支持",
    pathHint: "稍后支持",
    icon: "refresh",
    accent: "#59606d"
  },
  {
    id: "taterclient",
    name: "TaterClient",
    subtitle: "TClient",
    version: "第三方",
    status: "可添加",
    pathHint: "手动添加",
    icon: "gamepad",
    accent: "#5c6f58"
  },
  {
    id: "bestclient",
    name: "BestClient",
    subtitle: "BestProjectTeam",
    version: "第三方",
    status: "可添加",
    pathHint: "手动添加",
    icon: "toolKit",
    accent: "#755d67"
  },
  {
    id: "cactusclient",
    name: "Cactus Client",
    subtitle: "cactuss.top",
    version: "Third-party",
    status: "可添加",
    pathHint: "手动添加",
    icon: "wrench",
    accent: "#6f734e"
  },
  {
    id: "third-party",
    name: "第三方客户端",
    subtitle: "自定义",
    version: "自定义",
    status: "可添加",
    pathHint: "手动添加",
    icon: "toolKit",
    accent: "#4f5663"
  }
];

function localSmokeEnvEnabled(value: string | undefined) {
  return value?.trim() === "1";
}

function resolveLocalSmokeAutomation(): LocalSmokeAutomationConfig | null {
  if (!localSmokeEnvEnabled(import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE)) {
    return null;
  }

  return {
    clientInstallDir: import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR?.trim() ?? "",
    manifestUrl: import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL?.trim() ?? "",
    closeWindowOnFinish: localSmokeEnvEnabled(import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH)
  };
}

const localSmokeAutomation = resolveLocalSmokeAutomation();

function getUpdateRows(
  selectedClient: ClientInstallation | null,
  autoUpdateState: AutoUpdateState,
  autoUpdate: ClientUpdateCheck | null,
  autoUpdateError: string | null
) {
  const updateValue = (() => {
    switch (autoUpdateState) {
      case "disabled":
        return "未启用";
      case "idle":
        return "待检查";
      case "checking":
        return "检查中";
      case "available":
        return autoUpdate?.latest_version ? `可更新 ${autoUpdate.latest_version}` : "有更新";
      case "current":
        return "已是最新";
      case "manual":
        return "需打开来源";
      case "error":
        return autoUpdateError ?? "检查失败";
    }
  })();

  return [
    {
      icon: "cloudDownload" as const,
      label: "更新",
      value: updateValue,
      tone: autoUpdateState === "available" ? "text-[#e0b300]" : autoUpdateState === "error" ? "text-[#8f2f2f]" : "text-[#5f6673]"
    },
    { icon: "settings" as const, label: "网络", value: "未启用", tone: "text-[#5f6673]" },
    {
      icon: "folder" as const,
      label: "更新源",
      value: autoUpdate?.source_kind === "github_release" ? "GitHub" : autoUpdate?.source_kind === "website" ? "官网" : "内置",
      tone: "text-[#5f6673]"
    },
    {
      icon: "refresh" as const,
      label: "客户端",
      value: selectedClient?.version ? selectedClient.version : "未知",
      tone: "text-[#5f6673]"
    }
  ];
}

function ActivityBar(props: {
  activeView: AppView;
  onChangeView: (view: AppView) => void;
}) {
  const gamesActive = props.activeView === "games";

  return (
    <nav
      aria-label="主模块导航"
      className="relative z-50 flex h-full w-[88px] shrink-0 flex-col items-center overflow-visible bg-[linear-gradient(180deg,rgba(36,101,116,0.72)_0%,rgba(21,68,80,0.78)_42%,rgba(4,17,21,0.96)_100%)] px-3 py-5 text-white shadow-[22px_0_54px_rgba(0,0,0,0.20)] backdrop-blur-xl"
    >
      <button
        type="button"
        aria-label="DDNet Manager 首页"
        onClick={() => props.onChangeView("launch")}
        className="grid h-[58px] w-[58px] place-items-center rounded-full text-white transition hover:bg-white/10"
      >
        <img
          src={logoMark}
          alt=""
          className="h-10 w-10 select-none opacity-95 brightness-0 invert drop-shadow-[0_2px_10px_rgba(0,0,0,0.38)]"
        />
      </button>

      <div className="mt-16 flex flex-1 flex-col gap-4">
        {navItems.map((item) => {
          const active = props.activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              aria-current={active ? "page" : undefined}
              onClick={() => props.onChangeView(item.id)}
              className={`group relative grid h-[58px] w-[58px] place-items-center rounded-[18px] transition ${
                active
                  ? "bg-white/20 text-white shadow-[0_18px_30px_rgba(0,0,0,0.22)]"
                  : "text-white/64 hover:bg-white/10 hover:text-white"
              }`}
            >
              {active ? <span className="absolute -left-3 h-9 w-1.5 rounded-full bg-white" /> : null}
              <GameIcon name={item.icon} className="size-7" />
              <span className="pointer-events-none absolute left-[68px] z-[120] whitespace-nowrap rounded-xl bg-[#10171a] px-3 py-2 text-xs font-bold text-white opacity-0 shadow-xl transition group-hover:translate-x-1 group-hover:opacity-100">
                {item.label}
              </span>
            </button>
          );
        })}
      </div>

      <button
        type="button"
        aria-label="全部游戏"
        aria-current={gamesActive ? "page" : undefined}
        onClick={() => props.onChangeView("games")}
        className={`group relative grid h-[58px] w-[58px] place-items-center rounded-[18px] transition ${
          gamesActive
            ? "bg-white/20 text-white shadow-[0_18px_30px_rgba(0,0,0,0.22)]"
            : "text-white/64 hover:bg-white/10 hover:text-white"
        }`}
      >
        {gamesActive ? <span className="absolute -left-3 h-9 w-1.5 rounded-full bg-white" /> : null}
        <GameIcon name="gamepad" className="size-7" />
        <span className="pointer-events-none absolute left-[68px] z-[120] whitespace-nowrap rounded-xl bg-[#10171a] px-3 py-2 text-xs font-bold text-white opacity-0 shadow-xl transition group-hover:translate-x-1 group-hover:opacity-100">
          全部游戏
        </span>
      </button>
    </nav>
  );
}

const launcherNews = [
  "【更新】QmClient 下载与安装检测优化中",
  "【公告】DDNet Manager 首页视觉已切换为启动器模式"
];

function LaunchHero() {
  return (
    <motion.section
      initial={{ opacity: 0, x: -18 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ duration: 0.42, ease: "easeOut" }}
      className="absolute left-[72px] top-[82px] z-30 hidden w-[min(46vw,640px)] text-white min-[900px]:block"
    >
      <div className="text-[36px] font-black leading-none tracking-[0.02em] drop-shadow-[0_4px_22px_rgba(0,0,0,0.38)]">
        DDNet
      </div>
      <div className="mt-24 text-[26px] font-black text-white/82 drop-shadow-[0_3px_16px_rgba(0,0,0,0.42)]">
        QmClient-first 版本
      </div>
      <h1 className="mt-4 text-[clamp(48px,5vw,76px)] font-black leading-[0.96] tracking-[0] text-white/92 drop-shadow-[0_6px_26px_rgba(0,0,0,0.42)]">
        DDrace Network
      </h1>
      <button
        type="button"
        className="mt-8 h-16 min-w-[184px] rounded-[8px] border-2 border-white/72 bg-white/8 px-9 text-[20px] font-black text-white shadow-[0_12px_34px_rgba(0,0,0,0.20)] backdrop-blur-sm transition hover:-translate-y-0.5 hover:bg-white/18"
      >
        版本热点
      </button>
    </motion.section>
  );
}

function LaunchNewsPanel() {
  return (
    <motion.section
      aria-label="首页活动资讯"
      initial={{ opacity: 0, y: 18 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.08, duration: 0.42, ease: "easeOut" }}
      className="absolute bottom-[78px] left-[72px] z-20 hidden w-[min(40vw,610px)] overflow-hidden rounded-[12px] bg-[#05151a]/70 text-white shadow-[0_24px_60px_rgba(0,0,0,0.32)] backdrop-blur-md min-[1024px]:block"
    >
      <div className="h-[148px] overflow-hidden">
        <img
          src={launcherBackground}
          alt=""
          className="h-full w-full object-cover object-[50%_55%] opacity-86"
        />
      </div>
      <div className="px-7 pb-5 pt-5">
        <div className="flex items-center gap-9 text-[22px] font-black">
          <button type="button" className="relative text-white">
            活动
            <span className="absolute -bottom-3 left-1 h-1 w-6 rounded-full bg-[#ffd91f]" />
          </button>
          <button type="button" className="text-white/56 transition hover:text-white">公告</button>
          <button type="button" className="text-white/56 transition hover:text-white">资讯</button>
        </div>
        <div className="mt-7 grid gap-3 text-[16px] font-bold">
          {launcherNews.map((item) => (
            <div key={item} className="grid grid-cols-[minmax(0,1fr)_auto] gap-5">
              <span className="truncate text-white/92">{item}</span>
              <time className="text-white/84">06/07</time>
            </div>
          ))}
        </div>
      </div>
    </motion.section>
  );
}

const RIGHT_ACTION_ITEMS = [
  { label: "设置", icon: Disc3 },
  { label: "社群", icon: Share2 },
  { label: "消息", icon: MessageCircle },
  { label: "公告", icon: Radio },
  { label: "提醒", icon: Bell },
  { label: "帮助", icon: CircleHelp },
  { label: "客服", icon: Headphones }
] as const;

function RightActionRail() {
  return (
    <aside className="absolute right-5 top-[88px] z-40 hidden flex-col items-center gap-7 text-white/74 min-[1100px]:flex">
      {RIGHT_ACTION_ITEMS.map(({ label, icon: Icon }) => (
        <button
          key={label}
          type="button"
          aria-label={label}
          className="group relative grid h-9 w-9 place-items-center rounded-full drop-shadow-[0_3px_12px_rgba(0,0,0,0.42)] transition hover:-translate-y-0.5 hover:bg-white/10 hover:text-white"
        >
          <Icon size={28} strokeWidth={2.4} />
          <span className="pointer-events-none absolute right-11 whitespace-nowrap rounded-lg bg-[#10171a]/92 px-3 py-1.5 text-xs font-bold text-white opacity-0 shadow-xl transition group-hover:translate-x-[-2px] group-hover:opacity-100">
            {label}
          </span>
        </button>
      ))}
    </aside>
  );
}

function CompactUpdateStatus(props: {
  selectedClient: ClientInstallation | null;
  autoUpdateState: AutoUpdateState;
  autoUpdate: ClientUpdateCheck | null;
  autoUpdateError: string | null;
}) {
  return (
    <Card className="rounded-[28px] border-[var(--dm-border)] bg-white/72 text-[var(--dm-ink)] shadow-[0_18px_44px_rgba(47,52,64,0.08)] backdrop-blur-2xl">
      <CardHeader className="p-4 pb-0">
        <CardTitle className="text-[11px] font-black tracking-[0.18em] text-[#5f6673]">更新</CardTitle>
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-2 p-4 pt-3">
        {getUpdateRows(props.selectedClient, props.autoUpdateState, props.autoUpdate, props.autoUpdateError).map((row) => (
          <div key={row.label} className="flex min-w-0 items-center gap-2 rounded-2xl bg-[var(--dm-soft)] px-3 py-3">
            <GameIcon name={row.icon} className={`size-4 ${row.tone}`} />
            <div className="min-w-0">
              <div className="text-[9px] font-black uppercase tracking-[0.14em] text-[#8b9099]">{row.label}</div>
              <div className="truncate text-xs font-black text-[#3d4350]">{row.value}</div>
            </div>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

function WorkspaceShell(props: {
  activeView: AppView;
  children: ReactNode;
  customBackgroundUrl: string | null;
}) {
  const launchBackgroundUrl = props.customBackgroundUrl ?? launcherBackground;

  return (
    <section className="relative min-w-0 flex-1 overflow-hidden bg-[var(--dm-bg)]">
      <div className="absolute inset-0">
        {props.activeView === "launch" ? (
          <img
            src={launchBackgroundUrl}
            alt=""
            aria-hidden="true"
            className="h-full w-full object-cover object-[54%_50%]"
          />
        ) : null}
        {props.activeView !== "launch" && props.customBackgroundUrl ? (
          <img
            src={props.customBackgroundUrl}
            alt="自定义背景"
            className="h-full w-full object-cover opacity-28"
          />
        ) : null}
        {props.activeView === "launch" ? (
          <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(5,29,36,0.82)_0%,rgba(7,43,52,0.62)_30%,rgba(5,20,24,0.08)_58%,rgba(0,0,0,0.30)_100%),linear-gradient(0deg,rgba(0,0,0,0.52)_0%,rgba(0,0,0,0.08)_34%,rgba(0,0,0,0.12)_100%)]" />
        ) : (
          <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(243,241,234,0.96)_0%,rgba(243,241,234,0.90)_42%,rgba(243,241,234,0.84)_100%),radial-gradient(circle_at_78%_18%,rgba(47,52,64,0.06),transparent_28%)]" />
        )}
      </div>

      {props.activeView === "launch" ? <LaunchHero /> : null}
      {props.activeView === "launch" ? <LaunchNewsPanel /> : null}
      {props.activeView === "launch" ? <RightActionRail /> : null}

      <div className="relative z-10 flex h-full min-h-0 flex-col">
        {props.activeView === "launch" ? (
          <div className="min-h-0 flex-1 overflow-hidden px-4 pb-3 pt-4 md:px-6 md:pb-4 md:pt-6">
            {props.children}
          </div>
        ) : (
          <div className="dm-scroll min-h-0 flex-1 overflow-y-auto p-5 pt-16 md:p-7 md:pt-16">
            <div className="mx-auto max-w-[1120px] text-[var(--dm-ink)]">
              {props.children}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

export default function App() {
  const tauriRuntime = isTauriRuntime();
  const [activeView, setActiveView] = useState<AppView>("launch");
  const [customBackgroundUrl, setCustomBackgroundUrl] = useState<string | null>(null);
  const [backgroundMode, setBackgroundMode] = useState<BackgroundMode>("default");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [activeSettingsSection, setActiveSettingsSection] = useState<SettingsSectionId>("general");
  const { appSettings, savedAppSettings, settingsError, settingsState, changeSettings, saveSettings } = useAppSettings(tauriRuntime);
  const {
    clientPath,
    errorMessage,
    handleBackgroundValidationError,
    handleBrowse,
    handleClientPathChange,
    handlePrimaryAction,
    handleValidate,
    launchReadiness,
    launcherState,
    selectedClient,
    selectedClientTypeId,
    selectClientType
  } = useClientLauncher({
    appSettings,
    localSmokeAutomation,
    onOpenUpdateView: () => setActiveView("update"),
    tauriRuntime
  });
  const { autoUpdate, autoUpdateError, autoUpdateState } = useAutoUpdate({
    savedAppSettings,
    selectedClient,
    settingsState,
    tauriRuntime
  });

  const selectedClientType = clientTypes.find((client) => client.id === selectedClientTypeId) ?? clientTypes[0];

  const handleBackgroundImageSelect = async (file: File) => {
    if (!file.type.startsWith("image/")) {
      handleBackgroundValidationError("背景必须是图片。");
      return;
    }

    setCustomBackgroundUrl((current) => {
      if (current) {
        URL.revokeObjectURL(current);
      }
      return URL.createObjectURL(file);
    });
    setBackgroundMode("custom");
  };

  const clearBackgroundImage = () => {
    setCustomBackgroundUrl((current) => {
      if (current) {
        URL.revokeObjectURL(current);
      }
      return null;
    });
    setBackgroundMode("default");
  };

  useEffect(() => {
    return () => {
      if (customBackgroundUrl) {
        URL.revokeObjectURL(customBackgroundUrl);
      }
    };
  }, [customBackgroundUrl]);

  const changeView = (view: AppView) => {
    startTransition(() => setActiveView(view));
  };

  const changeSettingsSection = (section: SettingsSectionId) => {
    startTransition(() => setActiveSettingsSection(section));
  };

  const renderActiveView = () => {
    switch (activeView) {
      case "games":
        return (
          <div className="grid gap-6">
            <GamesPanel
              clientTypes={clientTypes}
              selectedClientTypeId={selectedClientTypeId}
              selectedClient={selectedClient}
              clientPath={clientPath}
              onSelectClientType={selectClientType}
            />
            <ClientManager />
          </div>
        );
      case "update":
        return <UpdatePanel smokeAutomation={localSmokeAutomation} />;
      case "launch":
        return (
          <LaunchPanel
            tauriRuntime={tauriRuntime}
            state={launcherState}
            clientPath={clientPath}
            readiness={launchReadiness}
            errorMessage={errorMessage}
            mobileUpdateStatus={
              <CompactUpdateStatus
                selectedClient={selectedClient}
                autoUpdateState={autoUpdateState}
                autoUpdate={autoUpdate}
                autoUpdateError={autoUpdateError}
              />
            }
            onBrowse={handleBrowse}
            onValidate={handleValidate}
            onPrimaryAction={handlePrimaryAction}
          />
        );
    }
  };

  return (
    <main className="relative h-screen w-screen overflow-hidden bg-[var(--dm-bg)] text-[var(--dm-muted-ink)]">
      <TitleBar onOpenSettings={() => setIsSettingsOpen(true)} />
      <section className="flex h-full min-h-0">
        <ActivityBar activeView={activeView} onChangeView={changeView} />
        <WorkspaceShell activeView={activeView} customBackgroundUrl={customBackgroundUrl}>
          {renderActiveView()}
        </WorkspaceShell>
      </section>
      <SettingsDialog
        open={isSettingsOpen}
        activeSection={activeSettingsSection}
        launcherState={launcherState}
        clientPath={clientPath}
        selectedClientType={selectedClientType}
        backgroundMode={backgroundMode}
        errorMessage={errorMessage}
        settings={appSettings}
        settingsState={settingsState}
        settingsError={settingsError}
        tauriRuntime={tauriRuntime}
        onClose={() => setIsSettingsOpen(false)}
        onSectionChange={changeSettingsSection}
        onSettingsChange={changeSettings}
        onSaveSettings={saveSettings}
        onClientPathChange={handleClientPathChange}
        onBrowse={handleBrowse}
        onValidate={handleValidate}
        onBackgroundImageSelect={handleBackgroundImageSelect}
        onClearBackgroundImage={clearBackgroundImage}
      />
    </main>
  );
}
