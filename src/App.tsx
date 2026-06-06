import { useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import {
  Boxes,
  Cloud,
  DownloadCloud,
  FolderSearch,
  Gamepad2,
  Globe2,
  HardDrive,
  Keyboard,
  Play,
  Radar,
  RotateCw,
  Server,
  ShieldCheck,
  Sparkles,
  Wrench
} from "lucide-react";
import ddnetArtwork from "./assets/ddnet2.svg";
import ddnetArtworkFallback from "./assets/ddnet2.webp";
import ddnetLogo from "./assets/logo.svg";
import { BindsPanel } from "./components/binds/BindsPanel";
import { ClientManager } from "./components/clients/ClientManager";
import { TitleBar } from "./components/layout/TitleBar";
import { LaunchPanel } from "./components/launch/LaunchPanel";
import { ResourcePanel } from "./components/resources/ResourcePanel";
import { UpdatePanel } from "./components/update/UpdatePanel";
import { launchClient, validateClientDir } from "./lib/tauri";
import type { ClientHealth, ClientInstallation, LauncherState } from "./types";

type AppView = "launch" | "clients" | "update" | "resources" | "binds";

type NavItem = {
  id: AppView;
  label: string;
  icon: ReactNode;
};

type WorkspaceBackground = {
  id: string;
  src: string;
  fallback: string;
  label: string;
};

const workspaceBackgrounds: WorkspaceBackground[] = [
  {
    id: "ddnet-default",
    src: ddnetArtwork,
    fallback: ddnetArtworkFallback,
    label: "DDNet Default"
  }
];

const navItems: NavItem[] = [
  { id: "launch", label: "启动", icon: <Play size={22} fill="currentColor" /> },
  { id: "clients", label: "客户端", icon: <Gamepad2 size={22} /> },
  { id: "update", label: "更新", icon: <DownloadCloud size={22} /> },
  { id: "resources", label: "资源", icon: <Boxes size={22} /> },
  { id: "binds", label: "Binds", icon: <Keyboard size={22} /> }
];

function normalizeHealth(health: ClientHealth): string {
  switch (health) {
    case "ok":
      return "目录健康";
    case "missing_executable":
      return "缺少 DDNet.exe";
    case "missing_storage_cfg":
      return "缺少 storage.cfg";
    case "missing_data_dir":
      return "缺少 data 目录";
  }
}

function getStatusText(state: LauncherState, client: ClientInstallation | null): string {
  switch (state) {
    case "unconfigured":
      return "等待配置";
    case "validating":
      return "验证中";
    case "ready":
      return client ? `准备就绪 · ${client.display_name}` : "准备就绪";
    case "launching":
      return "启动中";
    case "running":
      return "已拉起";
    case "error":
      return "配置错误";
  }
}

function ActivityBar(props: { activeView: AppView; onChangeView: (view: AppView) => void }) {
  return (
    <nav
      aria-label="主模块导航"
      className="flex h-full w-[72px] shrink-0 flex-col items-center border-r border-white/10 bg-[#12151f]/88 px-2 py-4 text-white shadow-[18px_0_60px_rgba(18,21,31,0.22)] backdrop-blur-2xl"
    >
      <div className="mb-5 grid h-11 w-11 place-items-center overflow-hidden rounded-2xl bg-white/92 shadow-[0_16px_34px_rgba(255,143,189,0.22)]">
        <img src={ddnetLogo} alt="DDNet Manager" className="h-7 w-7" />
      </div>

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
                  ? "bg-[linear-gradient(135deg,#ff86bd,#ffd777_58%,#8bdcff)] text-[#43233b] shadow-[0_14px_30px_rgba(255,137,189,0.32)]"
                  : "text-white/58 hover:bg-white/10 hover:text-white"
              }`}
            >
              {active ? <span className="absolute -left-2 h-7 w-1 rounded-full bg-[#ff9bc8]" /> : null}
              {item.icon}
              <span className="pointer-events-none absolute left-[58px] z-30 rounded-xl bg-[#151827]/95 px-3 py-2 text-xs font-bold text-white opacity-0 shadow-xl transition group-hover:translate-x-1 group-hover:opacity-100">
                {item.label}
              </span>
            </button>
          );
        })}
      </div>

      <button
        type="button"
        aria-label="设置"
        disabled
        title="设置暂未开放"
        className="grid h-12 w-12 place-items-center rounded-2xl text-white/22"
      >
        <Wrench size={21} />
      </button>
    </nav>
  );
}

function SidebarSection(props: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-3xl border border-white/12 bg-white/[0.07] p-4 shadow-[0_22px_60px_rgba(4,8,20,0.16)] backdrop-blur-2xl">
      <h3 className="text-[11px] font-black uppercase tracking-[0.22em] text-white/46">{props.title}</h3>
      <div className="mt-3">{props.children}</div>
    </section>
  );
}

function ContextSidebar(props: {
  activeView: AppView;
  clientPath: string;
  selectedClient: ClientInstallation | null;
  statusText: string;
}) {
  const activeLabel = navItems.find((item) => item.id === props.activeView)?.label ?? "启动";
  const recentClients = props.selectedClient
    ? [props.selectedClient.display_name, "DDNet Vanilla", "QmClient Nightly"]
    : ["QmClient Stable", "DDNet Vanilla", "QmClient Nightly"];

  return (
    <aside className="dm-scroll hidden h-full w-[232px] shrink-0 overflow-y-auto border-r border-white/12 bg-[#171b29]/78 px-4 py-5 text-white backdrop-blur-2xl min-[1024px]:block min-[1320px]:w-[292px]">
      <div className="mb-5">
        <div className="text-[11px] font-black uppercase tracking-[0.24em] text-[#ffaad0]">Workspace</div>
        <h2 className="mt-2 text-3xl font-black tracking-[-0.05em] text-white">{activeLabel}</h2>
      </div>

      {props.activeView === "launch" ? (
        <div className="space-y-4">
          <SidebarSection title="当前客户端">
            <div className="rounded-2xl bg-white/9 p-4">
              <div className="flex items-center gap-3">
                <div className="grid h-11 w-11 place-items-center rounded-2xl bg-[#ffe0ef] text-[#d85e93]">
                  <Gamepad2 size={21} />
                </div>
                <div className="min-w-0">
                  <div className="truncate text-base font-black text-white">
                    {props.selectedClient?.display_name ?? "未选择客户端"}
                  </div>
                  <div className="mt-1 text-xs font-semibold text-white/48">{props.statusText}</div>
                </div>
              </div>
              <div className="mt-4 rounded-xl bg-black/18 px-3 py-2 text-xs font-semibold leading-5 text-white/58">
                {props.selectedClient?.install_dir ?? (props.clientPath || "选择客户端目录后显示路径。")}
              </div>
            </div>
          </SidebarSection>

          <SidebarSection title="最近客户端">
            <div className="space-y-2">
              {recentClients.map((client, index) => (
                <div
                  key={`${client}-${index}`}
                  className={`w-full rounded-2xl px-3 py-3 text-left transition ${
                    index === 0
                      ? "bg-white/14 text-white"
                      : "bg-white/[0.055] text-white/58"
                  }`}
                >
                  <div className="flex items-center justify-between gap-3">
                    <span className="truncate text-sm font-black">{client}</span>
                    <span className="shrink-0 rounded-full bg-white/[0.08] px-2 py-1 text-[10px] font-black uppercase tracking-[0.12em] text-white/38">
                      soon
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </SidebarSection>

          <SidebarSection title="路径健康">
            <div className="grid gap-2 text-sm font-semibold text-white/68">
              <div className="flex items-center gap-2">
                <ShieldCheck size={16} className="text-[#a9f0c8]" />
                {props.selectedClient ? normalizeHealth(props.selectedClient.health) : "尚未验证"}
              </div>
              <div className="flex items-center gap-2">
                <HardDrive size={16} className="text-[#8bdcff]" />
                {props.selectedClient ? "已锁定启动目标" : "等待选择目录"}
              </div>
            </div>
          </SidebarSection>
        </div>
      ) : (
        <div className="space-y-4">
          <SidebarSection title="页面上下文">
            <div className="space-y-2 text-sm font-semibold leading-6 text-white/64">
              <div>当前模块会在右侧主工作区显示具体工具。</div>
              <div>侧栏后续可承载筛选、任务队列或文件树。</div>
            </div>
          </SidebarSection>
          <SidebarSection title="快捷入口">
            <div className="space-y-2">
              {["刷新状态", "打开目录", "查看日志"].map((label) => (
                <div
                  key={label}
                  className="w-full rounded-2xl bg-white/[0.045] px-3 py-3 text-left text-sm font-black text-white/38"
                >
                  {label} · 待接入
                </div>
              ))}
            </div>
          </SidebarSection>
        </div>
      )}
    </aside>
  );
}

function UpdateHub(props: { selectedClient: ClientInstallation | null; compact?: boolean }) {
  const rows = [
    { icon: <Globe2 size={16} />, label: "GitHub", value: "等待连通性检测", tone: "text-[#8bdcff]" },
    { icon: <Server size={16} />, label: "Host", value: "未启用加速", tone: "text-[#ffd777]" },
    { icon: <Cloud size={16} />, label: "Manifest", value: "未刷新", tone: "text-[#ff9bc8]" },
    {
      icon: <RotateCw size={16} />,
      label: "Client",
      value: props.selectedClient?.version ? `本地 ${props.selectedClient.version}` : "版本未知",
      tone: "text-[#a9f0c8]"
    }
  ];

  return (
    <section
      className={`rounded-[30px] border border-white/18 bg-[#141827]/68 p-5 text-white shadow-[0_30px_90px_rgba(6,10,26,0.28)] backdrop-blur-2xl ${
        props.compact ? "" : "w-[330px]"
      }`}
    >
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-[11px] font-black uppercase tracking-[0.24em] text-[#8bdcff]">Update Hub</div>
          <h3 className="mt-1 text-xl font-black tracking-[-0.03em]">网络与更新源</h3>
        </div>
        <div className="grid h-11 w-11 place-items-center rounded-2xl bg-white/12 text-[#ffd777]">
          <Radar size={20} />
        </div>
      </div>

      <div className="mt-5 grid gap-3">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center gap-3 rounded-2xl bg-white/[0.075] px-3 py-3">
            <div className={row.tone}>{row.icon}</div>
            <div className="min-w-0">
              <div className="text-[10px] font-black uppercase tracking-[0.2em] text-white/38">{row.label}</div>
              <div className="mt-0.5 truncate text-sm font-bold text-white/78">{row.value}</div>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function WorkspaceShell(props: {
  activeView: AppView;
  children: ReactNode;
  selectedClient: ClientInstallation | null;
}) {
  const background = workspaceBackgrounds[0];

  return (
    <section className="relative min-w-0 flex-1 overflow-hidden bg-[#090b12]">
      <div className="absolute inset-0">
        <img
          src={background.src}
          alt={background.label}
          className="h-full w-full object-cover opacity-64"
          onError={(event) => {
            event.currentTarget.src = background.fallback;
          }}
        />
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_72%_28%,rgba(255,213,231,0.16),transparent_24%),linear-gradient(90deg,rgba(8,10,18,0.86)_0%,rgba(8,10,18,0.48)_44%,rgba(8,10,18,0.24)_100%),linear-gradient(180deg,rgba(8,10,18,0.26),rgba(8,10,18,0.80))]" />
      </div>

      <div className="relative z-10 flex h-full min-h-0 flex-col">
        {props.activeView === "launch" ? (
          <div className="dm-scroll min-h-0 flex-1 overflow-y-auto p-5 md:p-7">
            {props.children}
          </div>
        ) : (
          <div className="dm-scroll min-h-0 flex-1 overflow-y-auto p-5 md:p-7">
            <div className="mx-auto max-w-[1120px] rounded-[34px] border border-white/18 bg-white/82 p-5 text-[#5f6782] shadow-[0_34px_90px_rgba(6,10,26,0.18)] backdrop-blur-2xl">
              {props.children}
            </div>
          </div>
        )}

        {props.activeView === "launch" ? (
          <div className="pointer-events-none absolute bottom-6 right-6 hidden min-[1320px]:block">
            <div className="pointer-events-auto">
              <UpdateHub selectedClient={props.selectedClient} />
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

export default function App() {
  const [activeView, setActiveView] = useState<AppView>("launch");
  const [launcherState, setLauncherState] = useState<LauncherState>("unconfigured");
  const [clientPath, setClientPath] = useState("");
  const [selectedClient, setSelectedClient] = useState<ClientInstallation | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const validationRequestId = useRef(0);

  const activeViewLabel = navItems.find((item) => item.id === activeView)?.label ?? "启动";

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
      markInvalid("请先选择客户端目录。");
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

      setSelectedClient(installation);
      setClientPath(installation.install_dir);

      if (installation.health !== "ok") {
        markInvalid(`目录已识别为 ${installation.display_name}，但当前不可启动：${normalizeHealth(installation.health)}。`, {
          clearClient: false
        });
        return;
      }

      setLauncherState("ready");
    } catch (error) {
      if (requestId !== validationRequestId.current) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      markInvalid(`目录验证失败：${message}`);
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
        title: "选择 DDNet / QmClient 客户端目录"
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
      markInvalid(`目录选择失败：${message}`);
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
      await launchClient(selectedClient.executable_path);
      setLauncherState("running");
      setErrorMessage(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      markInvalid(`启动失败：${message}`);
    }
  };

  const renderActiveView = () => {
    switch (activeView) {
      case "clients":
        return <ClientManager />;
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
            selectedClient={selectedClient}
            statusText={statusText}
            errorMessage={errorMessage}
            updateHub={<UpdateHub selectedClient={selectedClient} compact />}
            onClientPathChange={handleClientPathChange}
            onBrowse={handleBrowse}
            onValidate={handleValidate}
            onPrimaryAction={handlePrimaryAction}
          />
        );
    }
  };

  const statusText = getStatusText(launcherState, selectedClient);

  return (
    <main className="relative h-screen w-screen overflow-hidden bg-[#090b12] text-[#5f6782]">
      <TitleBar />
      <section className="flex h-full min-h-0 pt-11">
        <ActivityBar activeView={activeView} onChangeView={setActiveView} />
        <ContextSidebar
          activeView={activeView}
          clientPath={clientPath}
          selectedClient={selectedClient}
          statusText={statusText}
        />
        <WorkspaceShell activeView={activeView} selectedClient={selectedClient}>
          {activeView === "launch" ? renderActiveView() : (
            <div role="region" aria-label={`${activeViewLabel} 面板`}>
              {renderActiveView()}
            </div>
          )}
        </WorkspaceShell>
      </section>
    </main>
  );
}
