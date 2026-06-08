import { AnimatePresence, motion, useAnimationControls } from "framer-motion";
import { useState, type ReactNode } from "react";
import { GameIcon } from "@/components/icons/GameIcon";
import type { LauncherState, LaunchReadiness } from "../../types";

type LaunchButtonProps = {
  state: LauncherState;
  disabled: boolean;
  onClick: () => Promise<void>;
};

type LaunchPanelProps = {
  tauriRuntime: boolean;
  state: LauncherState;
  clientPath: string;
  readiness: LaunchReadiness | null;
  errorMessage: string | null;
  mobileUpdateStatus: ReactNode;
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

  const stateLabel = props.state === "validating" ? "启动，正在验证" : props.state === "launching" ? "启动中" : "启动";
  const buttonText =
    props.state === "launching" ? "启动中" : props.state === "validating" ? "验证中" : props.state === "ready" || props.state === "running" ? "开始游戏" : "获取游戏";

  return (
    <motion.button
      type="button"
      onClick={click}
      animate={controls}
      disabled={props.disabled}
      aria-label={stateLabel}
      whileHover={props.disabled ? undefined : { y: -3, scale: 1.01 }}
      whileTap={props.disabled ? undefined : { scale: 0.99 }}
      className="group relative flex h-[72px] w-[min(70vw,340px)] items-center justify-center gap-6 overflow-hidden rounded-full bg-[#ffd91f] px-5 text-center text-[#111213] shadow-[0_20px_46px_rgba(0,0,0,0.36)] transition-colors duration-200 hover:bg-[#202329] hover:text-[#ffd91f] disabled:cursor-not-allowed disabled:opacity-70 min-[1440px]:h-[78px] min-[1440px]:w-[370px]"
    >
      <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(255,255,255,0.36),rgba(255,255,255,0)_52%,rgba(17,18,19,0.08))]" />
      <span className="relative z-10 grid h-12 w-12 place-items-center rounded-full bg-[#111213] text-[#ffd91f] shadow-[inset_0_0_0_1px_rgba(255,255,255,0.08)] transition-colors duration-200 group-hover:bg-[#ffd91f] group-hover:text-[#111213]">
        <GameIcon name="cloudDownload" className="size-7 animate-[download-bob_1.2s_ease-in-out_infinite]" />
      </span>
      <span className="relative z-10 text-[26px] font-black tracking-[0]">{buttonText}</span>
    </motion.button>
  );
}

function LaunchReadinessCard(props: { readiness: LaunchReadiness | null }) {
  const readiness = props.readiness;

  if (!readiness) {
    return (
      <div className="w-[min(86vw,420px)] rounded-[18px] border border-white/12 bg-[#071113]/58 px-5 py-4 text-white shadow-[0_18px_46px_rgba(0,0,0,0.24)] backdrop-blur-md">
        <div className="text-xs font-black uppercase tracking-[0.18em] text-white/48">启动状态</div>
        <div className="mt-2 text-lg font-black text-white">读取默认客户端中</div>
        <p className="mt-1 text-sm font-bold text-white/62">正在从后端注册表读取默认客户端。</p>
      </div>
    );
  }

  const tone = readiness.running
    ? "border-[#41f2ff]/35 bg-[#41f2ff]/12 text-[#41f2ff]"
    : readiness.can_launch
      ? "border-[#ffd91f]/35 bg-[#ffd91f]/12 text-[#ffd91f]"
      : "border-[#ff6b6b]/30 bg-[#ff6b6b]/10 text-[#ff9a9a]";

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.22, ease: "easeOut" }}
      className="w-[min(86vw,420px)] overflow-hidden rounded-[18px] border border-white/12 bg-[#071113]/66 text-white shadow-[0_18px_46px_rgba(0,0,0,0.28)] backdrop-blur-md"
    >
      <div className="flex items-start justify-between gap-4 px-5 pt-4">
        <div className="min-w-0">
          <div className="text-xs font-black uppercase tracking-[0.18em] text-white/48">启动状态</div>
          <div className="mt-2 truncate text-xl font-black text-white">
            {readiness.client?.display_name ?? "未设置默认客户端"}
          </div>
          <p className="mt-1 text-sm font-bold text-white/66">{readiness.user_message}</p>
        </div>
        <span className={`shrink-0 rounded-full border px-3 py-1 text-xs font-black ${tone}`}>{readiness.status_label}</span>
      </div>

      {readiness.client ? (
        <div className="mt-4 grid grid-cols-2 gap-2 px-5 text-xs font-bold text-white/64">
          <div className="rounded-xl bg-white/8 px-3 py-2">
            <div className="text-white/38">版本</div>
            <div className="mt-1 truncate text-white/82">{readiness.client.version ?? "未知"}</div>
          </div>
          <div className="rounded-xl bg-white/8 px-3 py-2">
            <div className="text-white/38">来源</div>
            <div className="mt-1 truncate text-white/82">{readiness.client.install_source}</div>
          </div>
          <div className="col-span-2 rounded-xl bg-white/8 px-3 py-2">
            <div className="text-white/38">可执行文件</div>
            <div className="mt-1 truncate text-white/82">{readiness.client.executable_path}</div>
          </div>
        </div>
      ) : null}

      {readiness.blocking_reasons.length > 0 ? (
        <div className="mt-4 border-t border-white/10 px-5 py-3">
          <div className="text-xs font-black uppercase tracking-[0.16em] text-[#ffb0b0]">阻断项</div>
          <ul className="mt-2 grid gap-1 text-sm font-bold text-white/72">
            {readiness.blocking_reasons.map((reason) => (
              <li key={reason} className="flex gap-2">
                <span className="text-[#ff9a9a]">•</span>
                <span>{reason}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </motion.div>
  );
}

export function LaunchPanel(props: LaunchPanelProps) {
  const [isInstallDialogOpen, setIsInstallDialogOpen] = useState(false);
  const [desktopShortcut, setDesktopShortcut] = useState(true);
  const [startMenuShortcut, setStartMenuShortcut] = useState(true);
  const canBrowse = props.tauriRuntime;
  const canValidate = canBrowse && props.clientPath.trim().length > 0 && props.state !== "validating" && props.state !== "launching";
  const primaryDisabled = !props.tauriRuntime || props.state === "validating" || props.state === "launching";

  const handlePrimaryClick = async () => {
    if (primaryDisabled) {
      return;
    }

    if (props.state === "ready" || props.state === "running") {
      await props.onPrimaryAction();
      return;
    }

    setIsInstallDialogOpen(true);
  };

  return (
    <section className="mx-auto flex h-full w-full max-w-[1680px] items-end justify-center min-[900px]:justify-end">
      <motion.div
        initial={{ opacity: 0, y: 24 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.08, duration: 0.42, ease: "easeOut" }}
        className="grid w-full shrink-0 justify-items-center gap-4 pb-8 min-[900px]:justify-items-end min-[900px]:pr-[5.2vw]"
      >
        <div className="flex flex-col items-center gap-3 min-[900px]:items-end">
          <LaunchReadinessCard readiness={props.readiness} />
          <LaunchButton state={props.state} disabled={primaryDisabled} onClick={handlePrimaryClick} />
          <div className="flex flex-wrap items-center justify-center gap-x-3 gap-y-2 text-[22px] font-black">
            <span className="text-white drop-shadow-[0_2px_10px_rgba(0,0,0,0.42)]">已安装?</span>
            <button
              type="button"
              onClick={() => void props.onBrowse()}
              disabled={!canBrowse}
              className="text-[#ffd91f] drop-shadow-[0_2px_10px_rgba(0,0,0,0.36)] transition hover:-translate-y-0.5 hover:text-[#ffe866] disabled:cursor-not-allowed disabled:opacity-55 disabled:hover:translate-y-0 disabled:hover:text-[#ffd91f]"
            >
              定位游戏
            </button>
          </div>
        </div>
        {props.errorMessage ? (
          <div className="rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">
            {props.errorMessage}
          </div>
        ) : null}
        <div className="min-[1024px]:hidden">{props.mobileUpdateStatus}</div>
      </motion.div>
      <AnimatePresence>
        {isInstallDialogOpen ? (
          <motion.div
            className="fixed inset-0 z-[70] grid place-items-center bg-[#071113]/50 p-5 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onMouseDown={(event) => {
              if (event.target === event.currentTarget) {
                setIsInstallDialogOpen(false);
              }
            }}
          >
            <motion.section
              role="dialog"
              aria-modal="true"
              aria-label="选择安装路径"
              initial={{ opacity: 0, y: 22, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 14, scale: 0.98 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className="w-[min(92vw,620px)] rounded-[18px] border border-white/12 bg-[#202329] p-8 text-white shadow-[0_34px_120px_rgba(0,0,0,0.46)]"
            >
              <div className="flex items-start justify-between gap-5">
                <h2 className="text-2xl font-black tracking-[0]">选择安装路径</h2>
                <button
                  type="button"
                  aria-label="关闭安装路径弹窗"
                  onClick={() => setIsInstallDialogOpen(false)}
                  className="grid h-9 w-9 place-items-center rounded-full text-white/62 transition hover:bg-white/8 hover:text-white"
                >
                  ×
                </button>
              </div>

              <div className="mt-7 flex items-center gap-3 rounded-[12px] border border-white/12 bg-[#1a1d21] px-4 py-4">
                <span className="rounded-md bg-white/20 px-2 py-1 text-xs font-black text-white/80">SSD</span>
                <div className="min-w-0 flex-1 truncate text-sm font-bold text-white/78">
                  {props.clientPath || "请选择 DDNet / QmClient 安装目录"}
                </div>
                <button
                  type="button"
                  onClick={() => void props.onBrowse()}
                  disabled={!canBrowse}
                  className="text-sm font-black text-[#ffd91f] transition hover:text-[#ffe866] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:text-[#ffd91f]"
                >
                  更改
                </button>
              </div>

              <div className="mt-4 rounded-[12px] bg-[#2a2e34] p-4">
                <div className="flex items-center gap-3 text-sm font-bold text-white/72">
                  <GameIcon name="folder" className="size-5" />
                  安装目录
                  <span className="text-white/42">用于保存和更新客户端文件</span>
                </div>
                <div className="mt-3 flex items-center gap-3 text-sm font-bold text-white/72">
                  <GameIcon name="cloudDownload" className="size-5" />
                  更新包
                  <span className="text-white/42">下载后校验并安装</span>
                </div>
              </div>

              <div className="mt-5 grid gap-3 text-sm font-bold text-white/76">
                <button
                  type="button"
                  onClick={() => setDesktopShortcut((value) => !value)}
                  className="flex items-center gap-3 text-left"
                >
                  <span className={`grid h-5 w-5 place-items-center rounded ${desktopShortcut ? "bg-[#ffd91f] text-[#111213]" : "bg-white/12"}`}>
                    {desktopShortcut ? "✓" : ""}
                  </span>
                  创建桌面快捷方式
                </button>
                <button
                  type="button"
                  onClick={() => setStartMenuShortcut((value) => !value)}
                  className="flex items-center gap-3 text-left"
                >
                  <span className={`grid h-5 w-5 place-items-center rounded ${startMenuShortcut ? "bg-[#ffd91f] text-[#111213]" : "bg-white/12"}`}>
                    {startMenuShortcut ? "✓" : ""}
                  </span>
                  创建开始菜单快捷方式
                </button>
              </div>

              <div className="mt-8 flex flex-wrap items-center justify-end gap-5">
                <button
                  type="button"
                  onClick={() => void props.onBrowse()}
                  disabled={!canBrowse}
                  className="text-base font-black text-white/72 transition hover:text-white disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:text-white/72"
                >
                  已安装? <span className="text-white">定位游戏</span>
                </button>
                <button
                  type="button"
                  onClick={() => void props.onValidate()}
                  disabled={!canValidate}
                  className="h-12 rounded-[12px] bg-[#ffd91f] px-8 text-base font-black text-[#111213] transition hover:-translate-y-0.5 hover:bg-[#ffe866] disabled:cursor-not-allowed disabled:opacity-50"
                >
                  开始安装
                </button>
              </div>
            </motion.section>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </section>
  );
}
