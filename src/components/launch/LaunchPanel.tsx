import { motion, useAnimationControls } from "framer-motion";
import {
  AlertCircle,
  CheckCircle2,
  FolderOpen,
  Gamepad2,
  HardDrive,
  LoaderCircle,
  Play,
  Rocket,
  ShieldCheck,
  Sparkles
} from "lucide-react";
import type { ReactNode } from "react";
import type { ClientInstallation, LauncherState } from "../../types";

type LaunchButtonProps = {
  state: LauncherState;
  disabled: boolean;
  onClick: () => Promise<void>;
};

type LaunchPanelProps = {
  state: LauncherState;
  clientPath: string;
  selectedClient: ClientInstallation | null;
  statusText: string;
  errorMessage: string | null;
  updateHub: ReactNode;
  onClientPathChange: (value: string) => void;
  onBrowse: () => Promise<void>;
  onValidate: () => Promise<void>;
  onPrimaryAction: () => Promise<void>;
};

function LaunchButton(props: LaunchButtonProps) {
  const controls = useAnimationControls();

  const click = async () => {
    if (props.disabled) {
      return;
    }

    await controls.start({
      x: [0, -2, 2, 0],
      y: [0, -2, 0],
      scale: [1, 0.985, 1.012, 1],
      transition: { duration: 0.24, ease: "easeOut" }
    });

    await props.onClick();
  };

  const copy =
    props.state === "validating"
      ? {
          title: "验证中",
          subtitle: "检查目录结构和可执行文件",
          icon: <LoaderCircle className="animate-spin" size={22} />
        }
      : props.state === "ready"
        ? {
            title: "开始游戏",
            subtitle: "配置已验证，准备拉起客户端",
            icon: <Rocket size={22} />
          }
        : props.state === "launching"
          ? {
              title: "启动中",
              subtitle: "正在请求客户端进程",
              icon: <LoaderCircle className="animate-spin" size={22} />
            }
          : props.state === "running"
            ? {
                title: "再次请求启动",
                subtitle: "上次启动请求已发送，未监听进程",
                icon: <CheckCircle2 size={22} />
              }
            : props.state === "error"
              ? {
                  title: "重新配置",
                  subtitle: "修正路径后再启动",
                  icon: <AlertCircle size={22} />
                }
              : {
                  title: "选择客户端",
                  subtitle: "先指定 DDNet 或 QmClient 目录",
                  icon: <Play size={22} fill="currentColor" />
                };

  return (
    <motion.button
      type="button"
      onClick={click}
      animate={controls}
      disabled={props.disabled}
      whileHover={props.disabled ? undefined : { y: -3, scale: 1.01 }}
      whileTap={props.disabled ? undefined : { scale: 0.99 }}
      className="group relative flex min-h-[78px] w-full items-center justify-between overflow-hidden rounded-[999px] border border-[#ffec99]/70 bg-[#f5d328] px-5 text-left text-[#2c2510] shadow-[0_24px_70px_rgba(245,211,40,0.32)] transition disabled:cursor-not-allowed disabled:opacity-70 sm:w-[340px]"
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_20%_0%,rgba(255,255,255,0.9),transparent_32%),linear-gradient(135deg,rgba(255,255,255,0.45),rgba(255,255,255,0)_42%,rgba(0,0,0,0.12))]" />
      <div className="relative z-10 flex items-center gap-4">
        <div className="grid h-13 w-13 place-items-center rounded-full bg-[#161922] text-[#f5d328] shadow-[0_10px_26px_rgba(22,25,34,0.28)]">
          {copy.icon}
        </div>
        <div>
          <div className="text-[24px] font-black tracking-[-0.045em]">{copy.title}</div>
          <div className="mt-0.5 text-xs font-black uppercase tracking-[0.12em] opacity-70">{copy.subtitle}</div>
        </div>
      </div>
      <div className="relative z-10 hidden rounded-full bg-[#161922]/12 px-4 py-2 text-[11px] font-black uppercase tracking-[0.16em] sm:block">
        DDNet
      </div>
    </motion.button>
  );
}

function StatusChip(props: { icon: ReactNode; value: string; tone: "pink" | "mint" | "sky" }) {
  const toneClass =
    props.tone === "mint"
      ? "bg-[#a9f0c8]/14 text-[#c8ffe1] ring-[#a9f0c8]/20"
      : props.tone === "sky"
        ? "bg-[#8bdcff]/14 text-[#d6f6ff] ring-[#8bdcff]/20"
        : "bg-[#ff9bc8]/14 text-[#ffd9e9] ring-[#ff9bc8]/20";

  return (
    <div className={`inline-flex min-w-0 items-center gap-2 rounded-full px-3 py-2 text-sm font-black ring-1 ${toneClass}`}>
      <span className="shrink-0">
        {props.icon}
      </span>
      <span className="truncate">{props.value}</span>
    </div>
  );
}

export function LaunchPanel(props: LaunchPanelProps) {
  const canValidate = props.clientPath.trim().length > 0 && props.state !== "validating" && props.state !== "launching";
  const primaryDisabled = props.state === "validating" || props.state === "launching";

  return (
    <section className="mx-auto flex h-full w-full max-w-[1360px] flex-col justify-end gap-3">
      <motion.div
        initial={{ opacity: 0, y: 22 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.45, ease: "easeOut" }}
        className="max-w-[620px] shrink"
      >
        <div className="inline-flex items-center gap-2 rounded-full border border-white/16 bg-white/12 px-4 py-2 text-[11px] font-black uppercase tracking-[0.22em] text-[#ffb8d6] shadow-[0_18px_44px_rgba(6,10,26,0.18)] backdrop-blur-2xl">
          <Sparkles size={14} />
          Candy Utility Launcher
        </div>
        <h1 className="mt-4 text-[42px] font-black leading-[0.95] tracking-[-0.075em] text-white drop-shadow-[0_20px_60px_rgba(0,0,0,0.36)] sm:text-[56px] xl:text-[64px]">
          DDNet Manager
        </h1>
        <p className="mt-3 max-w-[520px] text-sm font-semibold leading-6 text-white/68">
          选择本地 DDNet / QmClient 目录，验证资源结构后直接启动。更新源和 Host 状态保留在右侧 Update Hub。
        </p>
      </motion.div>

      <motion.div
        initial={{ opacity: 0, y: 24 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.08, duration: 0.42, ease: "easeOut" }}
        className="grid shrink-0 gap-3 xl:grid-cols-[minmax(0,1fr)_340px]"
      >
        <div className="rounded-[30px] border border-white/16 bg-[#151927]/62 p-4 text-white shadow-[0_34px_100px_rgba(6,10,26,0.30)] backdrop-blur-2xl">
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-3">
                <div className="grid h-12 w-12 place-items-center rounded-2xl bg-[#ffe1f0] text-[#d85e93]">
                  <Gamepad2 size={23} />
                </div>
                <div className="min-w-0">
                  <div className="text-[11px] font-black uppercase tracking-[0.2em] text-white/42">当前客户端</div>
                  <div className="truncate text-2xl font-black tracking-[-0.04em] text-white">
                    {props.selectedClient?.display_name ?? "未配置客户端"}
                  </div>
                </div>
                <div className="rounded-full bg-white/12 px-3 py-1.5 text-xs font-black text-white/72">{props.statusText}</div>
              </div>

              <div className="mt-4 flex flex-wrap gap-2">
                <StatusChip
                  icon={<FolderOpen size={14} />}
                  value={props.selectedClient || props.clientPath ? "目录已填写" : "等待目录"}
                  tone="pink"
                />
                <StatusChip
                  icon={<ShieldCheck size={14} />}
                  value={props.selectedClient?.health === "ok" ? "目录健康" : "尚未验证"}
                  tone="mint"
                />
                <StatusChip
                  icon={<HardDrive size={14} />}
                  value={props.selectedClient?.executable_path ? "目标已锁定" : "未锁定目标"}
                  tone="sky"
                />
              </div>
            </div>

            <div className="flex items-end">
              <LaunchButton state={props.state} disabled={primaryDisabled} onClick={props.onPrimaryAction} />
            </div>
          </div>

          <div className="mt-4 rounded-[24px] border border-white/12 bg-white/[0.075] p-3">
            <label htmlFor="client-path" className="text-[10px] font-black uppercase tracking-[0.22em] text-white/42">
              客户端目录
            </label>
            <div className="mt-2 grid gap-2 lg:grid-cols-[minmax(0,1fr)_auto_auto]">
              <input
                id="client-path"
                value={props.clientPath}
                onChange={(event) => props.onClientPathChange(event.target.value)}
                placeholder="例如 D:/Games/QmClient"
                className="h-12 min-w-0 rounded-[18px] border border-white/14 bg-black/22 px-4 text-sm font-semibold text-white outline-none transition placeholder:text-white/34 focus:border-[#8bdcff] focus:ring-4 focus:ring-[#8bdcff]/20"
              />
              <button
                type="button"
                onClick={() => void props.onBrowse()}
                className="h-12 rounded-[18px] border border-white/12 bg-[#8bdcff]/18 px-5 text-sm font-black text-[#c9f2ff] transition hover:-translate-y-0.5 hover:bg-[#8bdcff]/26"
              >
                浏览目录
              </button>
              <button
                type="button"
                onClick={() => void props.onValidate()}
                disabled={!canValidate}
                className="h-12 rounded-[18px] border border-white/12 bg-[#ffd777]/18 px-5 text-sm font-black text-[#ffeaa5] transition hover:-translate-y-0.5 hover:bg-[#ffd777]/26 disabled:cursor-not-allowed disabled:opacity-55"
              >
                验证路径
              </button>
            </div>

            {props.errorMessage ? (
              <div className="mt-4 rounded-2xl border border-[#ff9bc8]/34 bg-[#ff9bc8]/14 px-4 py-3 text-sm font-semibold text-[#ffd9e9]">
                {props.errorMessage}
              </div>
            ) : null}
          </div>
        </div>

        <div className="min-[1320px]:hidden">{props.updateHub}</div>
      </motion.div>
    </section>
  );
}
