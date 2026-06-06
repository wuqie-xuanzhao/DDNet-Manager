import { useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import {
  Candy,
  FolderSearch,
  Gamepad2,
  HardDrive,
  LayoutPanelTop,
  Star
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

function MetricCard(props: {
  icon: ReactNode;
  label: string;
  value: string;
  tone: "mint" | "berry" | "lemon" | "sky";
}) {
  const toneClass =
    props.tone === "berry"
      ? "bg-[#ffe1f0] text-[#dc6498]"
      : props.tone === "lemon"
        ? "bg-[#fff3be] text-[#b68b11]"
        : props.tone === "sky"
          ? "bg-[#dff4ff] text-[#4a90be]"
          : "bg-[#dff9ed] text-[#3c9f74]";

  return (
    <div className="rounded-[28px] border border-white/80 bg-white/68 px-4 py-4 shadow-[0_18px_45px_rgba(134,164,214,0.12)] backdrop-blur-2xl">
      <div className="flex items-center gap-3">
        <div className={`grid h-11 w-11 place-items-center rounded-[16px] ${toneClass}`}>
          {props.icon}
        </div>
        <div>
          <div className="text-[11px] font-black uppercase tracking-[0.18em] text-[#99a5bd]">{props.label}</div>
          <div className="mt-1 text-sm font-bold text-[#5c6782]">{props.value}</div>
        </div>
      </div>
    </div>
  );
}

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

export default function App() {
  const [activeView, setActiveView] = useState<AppView>("launch");
  const [launcherState, setLauncherState] = useState<LauncherState>("unconfigured");
  const [clientPath, setClientPath] = useState("");
  const [selectedClient, setSelectedClient] = useState<ClientInstallation | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const validationRequestId = useRef(0);

  const navItems: Array<{ id: AppView; label: string }> = [
    { id: "launch", label: "启动" },
    { id: "clients", label: "客户端" },
    { id: "update", label: "更新" },
    { id: "resources", label: "资源" },
    { id: "binds", label: "Binds" }
  ];

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
      setLauncherState(value.trim() ? "unconfigured" : "unconfigured");
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
        return null;
    }
  };

  const statusText = getStatusText(launcherState, selectedClient);

  return (
    <main className="relative h-screen w-screen overflow-hidden bg-[var(--dm-bg)] text-[#5f6782]">
      <TitleBar />

      <div className="absolute inset-0 bg-[radial-gradient(circle_at_15%_18%,rgba(255,213,231,0.95),transparent_24%),radial-gradient(circle_at_82%_20%,rgba(183,238,255,0.92),transparent_28%),radial-gradient(circle_at_48%_88%,rgba(255,244,186,0.88),transparent_30%),linear-gradient(180deg,#fffdfa_0%,#f8fbff_48%,#fff7f1_100%)]" />
      <div className="absolute left-[-80px] top-[92px] h-[260px] w-[260px] rounded-full bg-[#ffd8ea]/70 blur-3xl" />
      <div className="absolute right-[-50px] top-[180px] h-[240px] w-[240px] rounded-full bg-[#c6efff]/72 blur-3xl" />
      <div className="absolute bottom-[-110px] left-[22%] h-[280px] w-[320px] rounded-full bg-[#fff1b6]/72 blur-3xl" />

      <section className="relative z-10 flex h-full pt-11">
        <div className="flex flex-1 flex-col overflow-hidden px-10 pb-6 pt-6">
          <div className="flex min-h-0 flex-1 gap-8">
            <div className="flex min-h-0 flex-1 flex-col">
              <motion.div
                initial={{ opacity: 0, y: 14 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.4, ease: "easeOut" }}
                className="inline-flex w-fit items-center gap-2 rounded-full bg-white/72 px-4 py-2 text-[11px] font-black uppercase tracking-[0.22em] text-[#e577a9] shadow-[0_14px_30px_rgba(255,184,208,0.24)]"
              >
                <Candy size={14} />
                Sugar Rush Shell
              </motion.div>

              <div className="mt-4 flex items-start justify-between gap-6">
                <div className="max-w-[640px]">
                  <motion.img
                    initial={{ opacity: 0, y: 12 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ delay: 0.05, duration: 0.45 }}
                    src={ddnetLogo}
                    alt="DDNet Manager logo"
                    className="h-11 w-auto"
                  />
                  <motion.h1
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ delay: 0.08, duration: 0.5 }}
                    className="mt-4 text-[52px] font-black leading-[0.92] tracking-[-0.07em] text-[#6a658c]"
                  >
                    糖果补给站
                    <span className="block bg-[linear-gradient(90deg,#ff73af_0%,#ffb168_42%,#55b8ff_100%)] bg-clip-text text-transparent">
                      QmClient Ready
                    </span>
                  </motion.h1>
                  <motion.p
                    initial={{ opacity: 0, y: 16 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ delay: 0.14, duration: 0.42 }}
                    className="mt-3 max-w-[620px] text-sm font-medium leading-7 text-[#7483a1]"
                  >
                    用奶油底、薄荷蓝、草莓粉和柠檬糖重新整理首页。主流程只保留一件事：找到你的客户端目录，验证成功后再真实拉起游戏。
                  </motion.p>
                </div>

                <motion.div
                  initial={{ opacity: 0, scale: 0.94, y: 18 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  transition={{ delay: 0.12, duration: 0.5 }}
                  className="relative hidden h-[210px] w-[250px] shrink-0 overflow-hidden rounded-[34px] border border-white/75 bg-white/55 shadow-[0_28px_70px_rgba(255,171,199,0.18)] backdrop-blur-2xl xl:block"
                >
                  <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(255,255,255,0.92),transparent_38%),linear-gradient(180deg,rgba(255,227,239,0.55),rgba(219,244,255,0.66))]" />
                  <img
                    src={ddnetArtwork}
                    alt="DDNet artwork"
                    className="relative z-10 h-full w-full object-contain p-5"
                    onError={(event) => {
                      event.currentTarget.src = ddnetArtworkFallback;
                    }}
                  />
                  <div className="absolute bottom-5 left-5 rounded-full bg-white/72 px-4 py-2 text-[11px] font-black uppercase tracking-[0.18em] text-[#7d87a0]">
                    Local Hero Asset
                  </div>
                </motion.div>
              </div>

              <motion.nav
                initial={{ opacity: 0, y: 16 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.18, duration: 0.42 }}
                className="mt-4 grid max-w-[760px] grid-cols-5 gap-3"
                aria-label="Manager 导航"
              >
                {navItems.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => setActiveView(item.id)}
                    aria-pressed={activeView === item.id}
                    aria-current={activeView === item.id ? "page" : undefined}
                    className={`rounded-[18px] border px-3 py-3 text-sm font-black tracking-[0.08em] transition ${
                      activeView === item.id
                        ? "border-white bg-white/84 text-[#5e6685] shadow-[0_14px_30px_rgba(255,182,211,0.24)]"
                        : "border-white/65 bg-white/45 text-[#89a0b9] hover:-translate-y-0.5 hover:bg-white/65"
                    } py-2.5`}
                  >
                    {item.label}
                  </button>
                ))}
              </motion.nav>

              <div className="dm-scroll mt-4 min-h-0 flex-1 overflow-y-auto pb-8 pr-3">
                {activeView === "launch" ? (
                  <LaunchPanel
                    state={launcherState}
                    clientPath={clientPath}
                    selectedClient={selectedClient}
                    statusText={statusText}
                    errorMessage={errorMessage}
                    onClientPathChange={handleClientPathChange}
                    onBrowse={handleBrowse}
                    onValidate={handleValidate}
                    onPrimaryAction={handlePrimaryAction}
                  />
                ) : (
                  <motion.div
                    initial={{ opacity: 0, y: 14 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.35, ease: "easeOut" }}
                    className="rounded-[36px] border border-white/80 bg-white/64 p-6 shadow-[0_24px_70px_rgba(150,182,220,0.14)] backdrop-blur-2xl"
                    role="region"
                    aria-label={`${activeViewLabel} 面板`}
                  >
                    {renderActiveView()}
                  </motion.div>
                )}
              </div>
            </div>

            <aside className="hidden w-[320px] shrink-0 xl:block">
              <div className="space-y-4">
                <MetricCard
                  icon={<FolderSearch size={18} />}
                  label="路径状态"
                  value={clientPath ? "已填写目录" : "等待选择"}
                  tone="sky"
                />
                <MetricCard
                  icon={<Gamepad2 size={18} />}
                  label="客户端健康"
                  value={selectedClient ? normalizeHealth(selectedClient.health) : "尚未验证"}
                  tone="berry"
                />
                <MetricCard
                  icon={<HardDrive size={18} />}
                  label="启动目标"
                  value={selectedClient ? selectedClient.executable_path : "未锁定可执行文件"}
                  tone="mint"
                />
                <MetricCard
                  icon={<LayoutPanelTop size={18} />}
                  label="当前状态"
                  value={statusText}
                  tone="lemon"
                />
              </div>

              <div className="mt-4 rounded-[32px] border border-white/80 bg-white/65 p-5 shadow-[0_20px_52px_rgba(255,192,217,0.16)] backdrop-blur-2xl">
                <div className="flex items-center gap-2 text-[11px] font-black uppercase tracking-[0.22em] text-[#90a1bf]">
                  <Star size={14} />
                  首页约束
                </div>
                <div className="mt-4 space-y-3 text-sm font-semibold leading-6 text-[#6f7d98]">
                  <div>不再展示 936MB 假包体。</div>
                  <div>不再监听 mock 下载事件。</div>
                  <div>没有有效目录时不伪装成可运行。</div>
                </div>
              </div>
            </aside>
          </div>
        </div>
      </section>
    </main>
  );
}
