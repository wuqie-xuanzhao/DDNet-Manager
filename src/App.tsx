import { startTransition, useEffect, useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import ddnetArt from "./assets/ddnet2.svg";
import ddnetLogo from "./assets/logo.svg";
import { BindsPanel } from "./components/binds/BindsPanel";
import { ClientManager } from "./components/clients/ClientManager";
import { GamesPanel, type ClientType, type ClientTypeId } from "./components/games/GamesPanel";
import { GameIcon, type GameIconName } from "./components/icons/GameIcon";
import { TitleBar } from "./components/layout/TitleBar";
import { LaunchPanel } from "./components/launch/LaunchPanel";
import { ResourcePanel } from "./components/resources/ResourcePanel";
import { SettingsDialog, type SettingsSectionId } from "./components/settings/SettingsDialog";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";
import { UpdatePanel } from "./components/update/UpdatePanel";
import { getDefaultClient, launchDefaultClient, upsertClientInstallation, validateClientDir } from "./lib/tauri";
import type { ClientHealth, ClientInstallation, LauncherState } from "./types";

type AppView = "launch" | "games" | "update" | "resources" | "binds";
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

function getUpdateRows(selectedClient: ClientInstallation | null) {
  return [
    { icon: "cloudDownload" as const, label: "下载源", value: "未检测", tone: "text-[#5f6673]" },
    { icon: "settings" as const, label: "网络", value: "未启用", tone: "text-[#5f6673]" },
    { icon: "folder" as const, label: "更新源", value: "未刷新", tone: "text-[#5f6673]" },
    {
      icon: "refresh" as const,
      label: "客户端",
      value: selectedClient?.version ? selectedClient.version : "未知",
      tone: "text-[#5f6673]"
    }
  ];
}

function normalizeHealth(health: ClientHealth): string {
  switch (health) {
    case "ok":
      return "可启动";
    case "missing_executable":
      return "缺少 DDNet.exe";
    case "missing_storage_cfg":
      return "缺少 storage.cfg";
    case "missing_data_dir":
      return "缺少 data 目录";
  }
}

function getStatusText(state: LauncherState, client: ClientInstallation | null, clientType: ClientType): string {
  switch (state) {
    case "unconfigured":
      return clientType.status;
    case "validating":
      return "验证中";
    case "ready":
      return client ? `就绪 · ${client.display_name}` : "就绪";
    case "launching":
      return "启动中";
    case "running":
      return "已启动";
    case "error":
      return "需处理";
  }
}

function ActivityBar(props: {
  activeView: AppView;
  onChangeView: (view: AppView) => void;
}) {
  const gamesActive = props.activeView === "games";

  return (
    <nav
      aria-label="主模块导航"
      className="relative z-50 flex h-full w-[76px] shrink-0 flex-col items-center overflow-visible border-r border-[var(--dm-border)] bg-[var(--dm-sidebar)] px-2 py-4 text-[var(--dm-ink)] shadow-[18px_0_50px_rgba(47,52,64,0.08)] backdrop-blur-2xl"
    >
      <button
        type="button"
        aria-label="返回启动页"
        onClick={() => props.onChangeView("launch")}
        className="mb-5 grid h-12 w-12 place-items-center overflow-hidden rounded-2xl border border-[var(--dm-border)] bg-white shadow-[0_14px_30px_rgba(47,52,64,0.08)] transition hover:-translate-y-0.5"
      >
        <img src={ddnetLogo} alt="DDNet Manager" className="h-8 w-8" />
      </button>

      <div className="flex flex-1 flex-col gap-2">
        {navItems.map((item) => {
          const active = props.activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              aria-label={item.label}
              aria-current={active ? "page" : undefined}
              onClick={() => props.onChangeView(item.id)}
              className={`group relative grid h-12 w-12 place-items-center rounded-2xl transition ${
                active
                  ? "bg-[var(--dm-ink)] text-[var(--dm-paper)] shadow-[0_14px_28px_rgba(47,52,64,0.18)]"
                  : "text-[var(--dm-muted-ink)] hover:bg-[var(--dm-soft)] hover:text-[var(--dm-ink)]"
              }`}
            >
              {active ? <span className="absolute -left-2 h-7 w-1 rounded-full bg-[var(--dm-ink)]" /> : null}
              <GameIcon name={item.icon} className="size-7" />
              <span className="pointer-events-none absolute left-[60px] z-[120] whitespace-nowrap rounded-xl bg-[var(--dm-ink)] px-3 py-2 text-xs font-bold text-white opacity-0 shadow-xl transition group-hover:translate-x-1 group-hover:opacity-100">
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
        className={`group relative grid h-12 w-12 place-items-center rounded-2xl transition ${
          gamesActive
            ? "bg-[var(--dm-ink)] text-[var(--dm-paper)] shadow-[0_14px_28px_rgba(47,52,64,0.18)]"
            : "text-[var(--dm-muted-ink)] hover:bg-[var(--dm-soft)] hover:text-[var(--dm-ink)]"
        }`}
      >
        {gamesActive ? <span className="absolute -left-2 h-7 w-1 rounded-full bg-[var(--dm-ink)]" /> : null}
        <GameIcon name="gamepad" className="size-7" />
        <span className="pointer-events-none absolute left-[60px] z-[120] whitespace-nowrap rounded-xl bg-[var(--dm-ink)] px-3 py-2 text-xs font-bold text-white opacity-0 shadow-xl transition group-hover:translate-x-1 group-hover:opacity-100">
          全部游戏
        </span>
      </button>
    </nav>
  );
}

function CompactUpdateStatus(props: { selectedClient: ClientInstallation | null }) {
  return (
    <Card className="rounded-[28px] border-[var(--dm-border)] bg-white/72 text-[var(--dm-ink)] shadow-[0_18px_44px_rgba(47,52,64,0.08)] backdrop-blur-2xl">
      <CardHeader className="p-4 pb-0">
        <CardTitle className="text-[11px] font-black tracking-[0.18em] text-[#5f6673]">更新</CardTitle>
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-2 p-4 pt-3">
        {getUpdateRows(props.selectedClient).map((row) => (
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
  return (
    <section className="relative min-w-0 flex-1 overflow-hidden bg-[var(--dm-bg)]">
      <div className="absolute inset-0">
        {props.customBackgroundUrl ? (
          <img
            src={props.customBackgroundUrl}
            alt="自定义背景"
            className="h-full w-full object-cover opacity-28"
          />
        ) : null}
        <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(243,241,234,0.96)_0%,rgba(243,241,234,0.90)_42%,rgba(243,241,234,0.84)_100%),radial-gradient(circle_at_78%_18%,rgba(47,52,64,0.06),transparent_28%)]" />
      </div>

      {props.activeView === "launch" ? (
        <img
          src={ddnetArt}
          alt=""
          aria-hidden="true"
          className="pointer-events-none absolute right-[5%] top-[8%] z-[1] hidden w-[min(46vw,540px)] select-none opacity-90 drop-shadow-[0_34px_70px_rgba(47,52,64,0.12)] min-[1024px]:block"
        />
      ) : null}

      <div className="relative z-10 flex h-full min-h-0 flex-col">
        {props.activeView === "launch" ? (
          <div className="min-h-0 flex-1 overflow-hidden px-4 pb-3 pt-4 md:px-6 md:pb-4 md:pt-6">
            {props.children}
          </div>
        ) : (
          <div className="dm-scroll min-h-0 flex-1 overflow-y-auto p-5 md:p-7">
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
  const [activeView, setActiveView] = useState<AppView>("launch");
  const [launcherState, setLauncherState] = useState<LauncherState>("unconfigured");
  const [clientPath, setClientPath] = useState("");
  const [selectedClient, setSelectedClient] = useState<ClientInstallation | null>(null);
  const [selectedClientTypeId, setSelectedClientTypeId] = useState<ClientTypeId>("qmclient");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [customBackgroundUrl, setCustomBackgroundUrl] = useState<string | null>(null);
  const [backgroundMode, setBackgroundMode] = useState<BackgroundMode>("default");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [activeSettingsSection, setActiveSettingsSection] = useState<SettingsSectionId>("general");
  const validationRequestId = useRef(0);

  const selectedClientType = clientTypes.find((client) => client.id === selectedClientTypeId) ?? clientTypes[0];

  const clientTypeIdFromInstallation = (client: ClientInstallation): ClientTypeId => {
    if (
      (client.client_id === "ddnet" || client.client_id === "ddnet_vanilla") &&
      client.install_dir.toLowerCase().includes("steamapps")
    ) {
      return "ddnet-steam";
    }

    switch (client.client_id) {
      case "qmclient":
        return "qmclient";
      case "qmclient_nightly":
        return "qmclient-nightly";
      case "ddnet":
        return "ddnet";
      case "ddnet_vanilla":
        return "ddnet";
      case "taterclient":
        return "taterclient";
      case "bestclient":
        return "bestclient";
      case "cactusclient":
        return "cactusclient";
      default:
        return "third-party";
    }
  };

  useEffect(() => {
    let alive = true;

    void getDefaultClient()
      .then((client) => {
        if (!alive || !client) {
          return;
        }

        setSelectedClient(client);
        setClientPath(client.install_dir);
        setSelectedClientTypeId(clientTypeIdFromInstallation(client));
        setLauncherState(client.health === "ok" ? "ready" : "error");
        setErrorMessage(client.health === "ok" ? null : `不可启动：${normalizeHealth(client.health)}。`);
      })
      .catch((error) => {
        if (!alive) {
          return;
        }
        setErrorMessage(error instanceof Error ? error.message : String(error));
      });

    return () => {
      alive = false;
    };
  }, []);

  const markInvalid = (message: string, options?: { clearClient?: boolean }) => {
    if (options?.clearClient ?? true) {
      setSelectedClient(null);
    }
    setLauncherState("error");
    setErrorMessage(message);
  };

  const validateClientPath = async (path: string) => {
    const normalizedPath = path.trim();
    if (!normalizedPath) {
      markInvalid("请先选择路径。");
      return;
    }

    setClientPath(normalizedPath);
    const requestId = validationRequestId.current + 1;
    validationRequestId.current = requestId;

    setLauncherState("validating");
    setErrorMessage(null);

    try {
      const installation = await validateClientDir(normalizedPath);
      if (requestId !== validationRequestId.current) {
        return;
      }

      if (installation.health !== "ok") {
        setSelectedClient(installation);
        setClientPath(installation.install_dir);
        markInvalid(`不可启动：${normalizeHealth(installation.health)}。`, {
          clearClient: false
        });
        return;
      }

      const savedInstallation = await upsertClientInstallation({
        install_dir: installation.install_dir,
        is_default: true
      });

      setSelectedClient(savedInstallation);
      setClientPath(savedInstallation.install_dir);
      setLauncherState("ready");
    } catch (error) {
      if (requestId !== validationRequestId.current) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      markInvalid(`验证失败：${message}`);
    }
  };

  const handleClientPathChange = (value: string) => {
    validationRequestId.current += 1;
    setClientPath(value);
    setErrorMessage(null);

    if (selectedClient && value !== selectedClient.install_dir) {
      setSelectedClient(null);
      setLauncherState("unconfigured");
    } else if (!value.trim()) {
      setSelectedClient(null);
      setLauncherState("unconfigured");
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择客户端目录"
      });

      if (typeof selected !== "string") {
        return;
      }

      setClientPath(selected);
      setSelectedClient(null);
      setErrorMessage(null);
      await validateClientPath(selected);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      markInvalid(`选择失败：${message}`);
    }
  };

  const handleValidate = async () => {
    await validateClientPath(clientPath);
  };

  const handlePrimaryAction = async () => {
    if (launcherState === "validating" || launcherState === "launching") {
      return;
    }

    if (!selectedClient || selectedClient.health !== "ok") {
      if (!clientPath.trim()) {
        await handleBrowse();
        return;
      }

      await handleValidate();
      return;
    }

    setLauncherState("launching");
    setErrorMessage(null);

    try {
      await launchDefaultClient();
      setLauncherState("running");
      setErrorMessage(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      markInvalid(`启动失败：${message}`);
    }
  };

  const handleBackgroundImageSelect = async (file: File) => {
    if (!file.type.startsWith("image/")) {
      markInvalid("背景必须是图片。", { clearClient: false });
      return;
    }

    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error("背景读取失败。"));
      reader.readAsDataURL(file);
    });

    setCustomBackgroundUrl(dataUrl);
    setBackgroundMode("custom");
  };

  const clearBackgroundImage = () => {
    setCustomBackgroundUrl(null);
    setBackgroundMode("default");
  };

  const changeView = (view: AppView) => {
    startTransition(() => setActiveView(view));
  };

  const changeSettingsSection = (section: SettingsSectionId) => {
    startTransition(() => setActiveSettingsSection(section));
  };

  const selectClientType = (id: ClientTypeId) => {
    if (id === selectedClientTypeId) {
      return;
    }

    startTransition(() => {
      setSelectedClientTypeId(id);
      setErrorMessage(null);
      setClientPath("");
      setSelectedClient(null);
      setLauncherState("unconfigured");
    });
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
        return <UpdatePanel />;
      case "resources":
        return <ResourcePanel />;
      case "binds":
        return <BindsPanel />;
      case "launch":
        return (
          <LaunchPanel
            state={launcherState}
            clientPath={clientPath}
            errorMessage={errorMessage}
            mobileUpdateStatus={<CompactUpdateStatus selectedClient={selectedClient} />}
            backgroundMode={backgroundMode}
            onClientPathChange={handleClientPathChange}
            onBrowse={handleBrowse}
            onValidate={handleValidate}
            onPrimaryAction={handlePrimaryAction}
            onBackgroundImageSelect={handleBackgroundImageSelect}
            onClearBackgroundImage={clearBackgroundImage}
          />
        );
    }
  };

  return (
    <main className="relative h-screen w-screen overflow-hidden bg-[var(--dm-bg)] text-[var(--dm-muted-ink)]">
      <TitleBar onOpenSettings={() => setIsSettingsOpen(true)} />
      <section className="flex h-full min-h-0 pt-11">
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
        onClose={() => setIsSettingsOpen(false)}
        onSectionChange={changeSettingsSection}
        onClientPathChange={handleClientPathChange}
        onBrowse={handleBrowse}
        onValidate={handleValidate}
        onBackgroundImageSelect={handleBackgroundImageSelect}
        onClearBackgroundImage={clearBackgroundImage}
      />
    </main>
  );
}
