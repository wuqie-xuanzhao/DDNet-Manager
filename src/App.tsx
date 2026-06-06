import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { motion } from "framer-motion";
import type { LauncherState } from "./types";
import { BindsPanel } from "./components/binds/BindsPanel";
import { ClientManager } from "./components/clients/ClientManager";
import { TitleBar } from "./components/layout/TitleBar";
import { LaunchButton, type DownloadProgressView } from "./components/launch/LaunchPanel";
import { ResourcePanel } from "./components/resources/ResourcePanel";
import { UpdatePanel } from "./components/update/UpdatePanel";
import {
  Activity,
  Gauge,
  HardDrive,
  Server,
  ShieldCheck,
  Zap
} from "lucide-react";

type VersionInfo = {
  current_version: string;
  latest_version: string;
  needs_update: boolean;
  channel: string;
  package_size_mb: number;
  notes: string[];
};

type AppView = "launch" | "clients" | "update" | "resources" | "binds";

function MetricCard(props: {
  icon: ReactNode;
  label: string;
  value: string;
  accent?: "cyan" | "amber" | "red";
}) {
  const accentClass =
    props.accent === "amber"
      ? "text-amber-300 shadow-amber-300/30"
      : props.accent === "red"
        ? "text-red-300 shadow-red-300/30"
        : "text-cyan-200 shadow-cyan-300/30";

  return (
    <div className="dm-glass dm-cut-corner relative overflow-hidden px-4 py-3">
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/22 to-transparent" />
      <div className="flex items-center gap-3">
        <div className={`grid h-9 w-9 place-items-center border border-white/10 bg-white/5 ${accentClass}`}>
          {props.icon}
        </div>
        <div>
          <div className="text-[10px] font-bold uppercase tracking-[0.28em] text-slate-400">{props.label}</div>
          <div className="mt-0.5 text-lg font-semibold tracking-wide text-slate-50">{props.value}</div>
        </div>
      </div>
    </div>
  );
}

export default function App() {
  const [activeView, setActiveView] = useState<AppView>("launch");
  const [launcherState, setLauncherState] = useState<LauncherState>("ready");
  const [versionInfo, setVersionInfo] = useState<VersionInfo | null>(null);
  const [patchInstalled, setPatchInstalled] = useState(false);
  const [progress, setProgress] = useState<DownloadProgressView>({
    progress: 0,
    downloaded_mb: 0,
    total_mb: 936,
    speed_mb_s: 0,
    status: "STANDBY"
  });

  useEffect(() => {
    let mounted = true;

    void invoke<VersionInfo>("check_update")
      .then((info) => {
        if (mounted) {
          setVersionInfo(info);
          setProgress((current) => ({
            ...current,
            total_mb: info.package_size_mb
          }));
        }
      })
      .catch((error: unknown) => {
        console.error("failed to check update", error);
      });

    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    void listen<DownloadProgressView>("download-progress", (event) => {
      setProgress(event.payload);

      if (event.payload.progress >= 100) {
        setPatchInstalled(true);
        setLauncherState("ready");
        setVersionInfo((current) => {
          if (!current) {
            return current;
          }

          return {
            ...current,
            current_version: current.latest_version,
            needs_update: false
          };
        });
      }
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const hasPendingUpdate = Boolean(versionInfo?.needs_update && !patchInstalled);
  const navItems: Array<{ id: AppView; label: string }> = [
    { id: "launch", label: "启动" },
    { id: "clients", label: "客户端" },
    { id: "update", label: "更新" },
    { id: "resources", label: "资源" },
    { id: "binds", label: "Binds" }
  ];
  const activeViewLabel = navItems.find((item) => item.id === activeView)?.label ?? "启动";

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

  const handlePrimaryAction = async () => {
    if (launcherState === "running" || launcherState === "downloading") {
      return;
    }

    if (hasPendingUpdate) {
      setProgress({
        progress: 0,
        downloaded_mb: 0,
        total_mb: versionInfo?.package_size_mb ?? 936,
        speed_mb_s: 0,
        status: "INITIALIZING"
      });
      setLauncherState("downloading");
      await invoke("start_download");
      return;
    }

    await invoke("launch_game");
    setLauncherState("running");
  };

  return (
    <main className="dm-grid dm-noise dm-scanline relative h-screen w-screen overflow-hidden bg-[#111213] text-slate-100">
      <TitleBar />

      <div className="absolute inset-0 bg-[radial-gradient(circle_at_22%_24%,rgba(65,242,255,0.18),transparent_28%),radial-gradient(circle_at_70%_20%,rgba(255,63,79,0.12),transparent_25%),linear-gradient(135deg,#111213_0%,#090a0b_52%,#15191c_100%)]" />
      <div className="absolute left-[-180px] top-[120px] h-[520px] w-[760px] rotate-[-18deg] border border-cyan-300/10 bg-cyan-200/4 blur-[1px]" />
      <div className="absolute bottom-[-180px] right-[-80px] h-[420px] w-[680px] rotate-[-9deg] border border-white/8 bg-white/4" />

      <section className="relative z-10 flex h-full pt-11">
        <div className="relative flex flex-1 flex-col overflow-hidden px-12 pb-32 pt-10">
          <div className="flex min-h-0 flex-1 flex-col">
            <div className="min-h-0 flex-1 overflow-y-auto pr-2">
              <motion.div
                initial={{ opacity: 0, y: 18 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.45, ease: "easeOut" }}
                className="inline-flex items-center gap-3 border border-cyan-100/14 bg-black/28 px-4 py-2 text-[11px] font-bold uppercase tracking-[0.32em] text-cyan-100/78"
              >
                <span className="h-1.5 w-1.5 rounded-full bg-cyan-300 shadow-[0_0_16px_rgba(65,242,255,1)]" />
                Stable Channel · Precision Launch Protocol
              </motion.div>

              <motion.h1
                initial={{ opacity: 0, y: 28 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.08, duration: 0.55, ease: "easeOut" }}
                className="mt-9 max-w-[720px] text-[86px] font-black uppercase leading-[0.88] tracking-[-0.055em] text-white"
              >
                DDNet
                <span className="block text-cyan-100 drop-shadow-[0_0_28px_rgba(65,242,255,0.28)]">Manager</span>
              </motion.h1>

              <motion.p
                initial={{ opacity: 0, y: 18 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.16, duration: 0.45, ease: "easeOut" }}
                className="mt-6 max-w-[560px] text-sm font-medium leading-7 tracking-[0.08em] text-slate-300/78"
              >
                Industrial-grade launch shell for hardline DDrace sessions. Verify assets, deploy client packages and hand off execution with a clean mechanical rhythm.
              </motion.p>

              <motion.nav
                initial={{ opacity: 0, y: 16 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2, duration: 0.4, ease: "easeOut" }}
                className="relative z-10 mt-8 grid max-w-[720px] grid-cols-5 gap-2"
                aria-label="Manager 导航"
              >
                {navItems.map((item) => (
                  <button
                    key={item.id}
                    onClick={() => setActiveView(item.id)}
                    aria-pressed={activeView === item.id}
                    aria-current={activeView === item.id ? "page" : undefined}
                    className={`border px-3 py-2 text-xs font-black uppercase tracking-[0.18em] transition ${
                      activeView === item.id
                        ? "border-cyan-200 bg-cyan-200 text-slate-950"
                        : "border-white/10 bg-white/5 text-slate-300 hover:border-cyan-200/40"
                    }`}
                  >
                    {item.label}
                  </button>
                ))}
              </motion.nav>

              {activeView !== "launch" ? (
                <motion.div
                  initial={{ opacity: 0, y: 16 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.24, duration: 0.4, ease: "easeOut" }}
                  className="mt-5 max-w-[720px]"
                  role="region"
                  aria-label={`${activeViewLabel} 面板`}
                >
                  {renderActiveView()}
                </motion.div>
              ) : null}
            </div>

            <div className="mt-6 grid w-[660px] shrink-0 grid-cols-3 gap-3">
              <MetricCard icon={<Server size={18} />} label="Mirror" value="CN-AUTO" />
              <MetricCard icon={<Gauge size={18} />} label="Latency" value="24 ms" />
              <MetricCard icon={<ShieldCheck size={18} />} label="Integrity" value="SEALED" />
            </div>
          </div>

          <div className="absolute bottom-0 left-0 right-[420px] h-28 border-t border-white/10 bg-black/24 backdrop-blur-xl">
            <div className="flex h-full items-center justify-between px-12">
              <div>
                <div className="text-[10px] font-black uppercase tracking-[0.34em] text-slate-500">Client Version</div>
                <div className="mt-2 text-2xl font-bold text-slate-100">
                  {versionInfo ? versionInfo.current_version : "Scanning"}
                  <span className="ml-3 text-sm font-semibold text-cyan-200/80">
                    {hasPendingUpdate && versionInfo ? `Latest ${versionInfo.latest_version}` : "Synchronized"}
                  </span>
                </div>
              </div>

              <LaunchButton
                state={launcherState}
                progress={progress}
                hasPendingUpdate={hasPendingUpdate}
                onClick={handlePrimaryAction}
              />
            </div>
          </div>
        </div>

        <aside className="relative h-full w-[420px] border-l border-white/10 bg-black/28 px-7 pb-8 pt-8 backdrop-blur-2xl">
          <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(65,242,255,0.08),transparent_34%,rgba(255,63,79,0.06))]" />

          <div className="relative z-10 mt-4">
            <div className="dm-glass dm-cut-corner h-[252px] overflow-hidden p-5">
              <div className="relative h-full border border-cyan-200/12 bg-[#0a0d0f]">
                <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_20%,rgba(65,242,255,0.22),transparent_32%),linear-gradient(145deg,rgba(255,255,255,0.07),rgba(255,255,255,0))]" />
                <div className="absolute inset-5 border border-white/10" />
                <div className="absolute bottom-5 left-5 right-5">
                  <div className="text-[10px] font-black uppercase tracking-[0.34em] text-cyan-100/58">Visual Slot</div>
                  <div className="mt-2 text-2xl font-black uppercase tracking-[-0.02em]">Hero Media Reserved</div>
                  <div className="mt-2 text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
                    Background video or seasonal DDNet artwork
                  </div>
                </div>
              </div>
            </div>

            <div className="mt-5 space-y-3">
              <MetricCard
                icon={<HardDrive size={18} />}
                label="Package"
                value={`${progress.downloaded_mb.toFixed(1)} / ${progress.total_mb.toFixed(1)} MB`}
                accent="cyan"
              />
              <MetricCard
                icon={<Zap size={18} />}
                label="Throughput"
                value={launcherState === "downloading" ? `${progress.speed_mb_s.toFixed(1)} MB/s` : "Standby"}
                accent="amber"
              />
              <MetricCard
                icon={<Activity size={18} />}
                label="State"
                value={launcherState === "running" ? "Running" : launcherState === "downloading" ? progress.status : "Ready"}
                accent={launcherState === "running" ? "red" : "cyan"}
              />
            </div>

            <div className="dm-glass dm-cut-corner mt-5 p-5">
              <div className="flex items-center justify-between">
                <div className="text-[11px] font-black uppercase tracking-[0.3em] text-slate-400">Patch Notes</div>
                <div className="text-[11px] font-black uppercase tracking-[0.24em] text-cyan-200">
                  {versionInfo?.channel ?? "stable"}
                </div>
              </div>

              <div className="mt-4 space-y-3">
                {(versionInfo?.notes ?? ["Checking update metadata.", "Preparing launcher shell.", "Waiting for backend handshake."]).map((note) => (
                  <div key={note} className="flex gap-3 border-l border-cyan-200/20 pl-3 text-sm font-medium leading-5 text-slate-300/84">
                    <span className="mt-2 h-1 w-1 shrink-0 rounded-full bg-cyan-300" />
                    <span>{note}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </aside>
      </section>
    </main>
  );
}
