import { motion, useAnimationControls } from "framer-motion";
import { Activity, Download, Play } from "lucide-react";
import type { LauncherState } from "../../types";

export type DownloadProgressView = {
  progress: number;
  downloaded_mb: number;
  total_mb: number;
  speed_mb_s: number;
  status: string;
};

export function LaunchButton(props: {
  state: LauncherState;
  progress: DownloadProgressView;
  hasPendingUpdate: boolean;
  onClick: () => Promise<void>;
}) {
  const controls = useAnimationControls();

  const click = async () => {
    await controls.start({
      x: [0, -3, 4, -2, 2, 0],
      scale: [1, 0.985, 1.012, 0.996, 1],
      transition: { duration: 0.24, ease: "easeOut" }
    });

    await props.onClick();
  };

  const title =
    props.state === "running"
      ? "游戏运行中"
      : props.state === "downloading"
        ? "部署中"
        : "开始游戏";

  const subtitle =
    props.state === "running"
      ? "DDNet client process handoff active"
      : props.state === "downloading"
        ? `${props.progress.status} · ${props.progress.speed_mb_s.toFixed(1)} MB/s`
        : props.hasPendingUpdate
          ? "启动前将自动部署最新稳定资源"
          : "配置已验证，客户端准备就绪";

  return (
    <motion.button
      onClick={click}
      animate={controls}
      disabled={props.state === "downloading"}
      whileHover={props.state === "downloading" ? undefined : { y: -2 }}
      whileTap={props.state === "downloading" ? undefined : { scale: 0.992 }}
      className="group relative h-[92px] w-[390px] overflow-hidden border border-cyan-200/35 bg-cyan-300 text-left text-slate-950 shadow-[0_0_42px_rgba(65,242,255,0.28)] transition disabled:cursor-default disabled:border-cyan-200/18 disabled:bg-slate-800 disabled:text-cyan-100"
      style={{
        clipPath: "polygon(24px 0, 100% 0, 100% calc(100% - 24px), calc(100% - 24px) 100%, 0 100%, 0 24px)"
      }}
    >
      <div className="absolute inset-0 bg-[linear-gradient(120deg,rgba(255,255,255,0.55),rgba(255,255,255,0)_28%,rgba(0,0,0,0.18)_72%,rgba(0,0,0,0.42))]" />
      <div className="absolute inset-y-0 right-0 w-28 bg-black/12 transition group-hover:w-36" />

      {props.state === "downloading" && (
        <motion.div
          className="absolute inset-y-0 left-0 bg-cyan-300/22"
          initial={{ width: 0 }}
          animate={{ width: `${props.progress.progress}%` }}
          transition={{ duration: 0.16, ease: "linear" }}
        />
      )}

      <div className="relative z-10 flex h-full items-center justify-between px-8">
        <div>
          <div className="flex items-center gap-3">
            {props.state === "running" ? <Activity size={25} /> : props.state === "downloading" ? <Download size={25} /> : <Play size={25} fill="currentColor" />}
            <span className="text-3xl font-black uppercase tracking-[0.08em]">{title}</span>
          </div>
          <div className="mt-2 text-[11px] font-bold uppercase tracking-[0.24em] opacity-70">{subtitle}</div>
        </div>

        <div className="text-right">
          <div className="text-[10px] font-black uppercase tracking-[0.28em] opacity-60">Core</div>
          <div className="mt-1 text-2xl font-black tabular-nums">
            {props.state === "downloading" ? `${props.progress.progress}%` : "01"}
          </div>
        </div>
      </div>

      <div className="absolute bottom-0 left-0 h-1 w-full bg-black/28">
        <motion.div
          className="h-full bg-slate-950"
          animate={{ width: props.state === "downloading" ? `${props.progress.progress}%` : props.state === "running" ? "100%" : "0%" }}
          transition={{ duration: 0.18, ease: "linear" }}
        />
      </div>
    </motion.button>
  );
}
