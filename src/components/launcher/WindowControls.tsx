import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Download, Loader2, AlertCircle, X, ExternalLink, RefreshCw } from "lucide-react";
import type { useAppUpdater } from "@/hooks/useAppUpdater";

interface WindowControlsProps {
  onOpenSettings: () => void;
  onCloseLauncher: () => void;
  isAudioOn: boolean;
  onToggleAudio: () => void;
  onMinimize: () => void;
  /// 启动器自身更新检查 hook 返回值。undefined 时不渲染更新按钮。
  appUpdater?: ReturnType<typeof useAppUpdater>;
}

export default function WindowControls({
  onOpenSettings,
  onCloseLauncher,
  isAudioOn,
  onToggleAudio,
  onMinimize,
  appUpdater
}: WindowControlsProps) {
  return (
    <div className="absolute top-5 right-7 flex items-center space-x-1.5 z-50 select-none">
      {/* 启动器更新按钮：和声音/设置/最小化/关闭并列。仅在 appUpdater.visible 时渲染 */}
      {appUpdater && <UpdateControlButton updater={appUpdater} />}

      {/* Sound Controller Button representing atmosphere status */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-bgm-sound-controller-top"
          type="button"
          aria-label={isAudioOn ? "静音背景乐" : "播放背景乐"}
          onClick={onToggleAudio}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-[#fed330] hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          {isAudioOn ? (
            <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M11 5L6 9H2v6h4l5 4V5z" fill="currentColor" />
              <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
              <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none opacity-60" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M11 5L6 9H2v6h4l5 4V5z" />
              <line x1="23" y1="9" x2="17" y2="15" />
              <line x1="17" y1="9" x2="23" y2="15" />
            </svg>
          )}
        </motion.button>

        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10">{isAudioOn ? "静音背景乐" : "播放背景乐"}</span>
          </div>
        </div>
      </div>

      {/* Settings Button */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-settings"
          type="button"
          aria-label="设置"
          onClick={onOpenSettings}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-white hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 2L20.66 7V17L12 22L3.34 17V7L12 2Z" />
            <circle cx="12" cy="12" r="3.2"/>
          </svg>
        </motion.button>

        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10 tracking-wide">设置</span>
          </div>
        </div>
      </div>

      {/* Minimize Button */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-minimize"
          type="button"
          aria-label="最小化"
          onClick={onMinimize}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-white hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.8" strokeLinecap="round">
            <line x1="4" y1="12" x2="20" y2="12" />
          </svg>
        </motion.button>

        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10">最小化</span>
          </div>
        </div>
      </div>

      {/* Close Button */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-close"
          type="button"
          aria-label="关闭"
          onClick={onCloseLauncher}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-red-400 hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </motion.button>

        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10">关闭</span>
          </div>
        </div>
      </div>
    </div>
  );
}

/// 启动器更新检查按钮。视觉与其他 4 个控制按钮一致（w-8 h-8 rounded-md），
/// 多了"发现新版本"时的红点 + 点击展开卡片显示版本信息。
function UpdateControlButton({ updater }: { updater: ReturnType<typeof useAppUpdater> }) {
  const { state, updateInfo, error, checkForUpdate } = updater;
  const [open, setOpen] = useState(false);

  if (!updater.visible) return null;

  const tooltipText = (() => {
    switch (state) {
      case "checking": return "检查更新中…";
      case "up-to-date": return "启动器已是最新";
      case "has-update": return `发现新版本 v${updateInfo?.latest_version ?? "?"}`;
      case "failed": return "检查更新失败";
      default: return "检查启动器更新";
    }
  })();

  // 视觉编码：默认（idle/up-to-date）灰色和其他按钮一致；has-update 才变绿色 + 红点提示；
  // checking/failed 用对应状态图标。颜色和形状和其他 4 个按钮协调（w-8 h-8 rounded-md）。
  const iconColor = state === "has-update"
    ? "text-emerald-400"
    : "text-[#cccccc]";
  const hoverColor = state === "has-update"
    ? "hover:text-emerald-300"
    : "hover:text-white";

  const handleClick = () => {
    if (state === "has-update" || state === "failed") {
      setOpen((v) => !v);
    } else {
      void checkForUpdate({ force: true });
    }
  };

  return (
    <div className="relative group flex flex-col items-center">
      <motion.button
        id="btn-app-update"
        type="button"
        aria-label={tooltipText}
        onClick={handleClick}
        whileTap={{ scale: 0.92 }}
        className={`${iconColor} ${hoverColor} hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md relative`}
      >
        {state === "checking" ? (
          <Loader2 className="w-[18px] h-[18px] animate-spin" />
        ) : state === "failed" ? (
          <AlertCircle className="w-[18px] h-[18px] text-red-400" />
        ) : (
          <>
            {/* 默认 / has-update 都用下载图标，差异在外圈颜色和红点 */}
            <Download className="w-[18px] h-[18px]" />
            {state === "has-update" && (
              <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            )}
          </>
        )}
      </motion.button>

      {/* 与其他 4 个按钮一致的白底 tooltip（hover 时显示） */}
      <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
        <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
          <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
          <span className="relative z-10">{tooltipText}</span>
        </div>
      </div>

      {/* 点击展开的卡片（仅 has-update / failed 时） */}
      <AnimatePresence>
        {open && (state === "has-update" || state === "failed") && (
          <>
            <div
              className="fixed inset-0 z-40"
              onClick={() => setOpen(false)}
            />
            <motion.div
              initial={{ opacity: 0, scale: 0.95, y: -8 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: -8 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className="absolute top-[170%] right-0 z-50 w-80 bg-[var(--app-surface)] border border-[var(--app-border)] rounded-xl shadow-[0_12px_40px_rgba(0,0,0,0.6)] overflow-hidden"
            >
              <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--app-border-subtle)]">
                <span className="text-xs font-bold uppercase tracking-wider text-[var(--app-text-muted)]">
                  启动器更新
                </span>
                <button
                  type="button"
                  aria-label="关闭"
                  onClick={() => setOpen(false)}
                  className="w-6 h-6 rounded-md flex items-center justify-center text-[var(--app-text-muted)] hover:text-[var(--app-text)] hover:bg-white/5 transition-colors cursor-pointer"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>

              <div className="p-4 space-y-3">
                {state === "has-update" && updateInfo ? (
                  <>
                    <div className="flex items-center gap-2">
                      <div className="w-2 h-2 rounded-full bg-[var(--app-accent)] animate-pulse" />
                      <span className="text-sm font-bold text-[var(--app-text)]">
                        发现新版本 v{updateInfo.latest_version}
                      </span>
                    </div>
                    <div className="text-[11px] text-[var(--app-text-muted)] font-mono">
                      当前 v{updateInfo.current_version}
                    </div>

                    {updateInfo.release_notes && (
                      <div className="max-h-32 overflow-y-auto p-2.5 bg-[var(--app-sunken)] border border-[var(--app-border-subtle)] rounded-lg">
                        <pre className="text-[11px] font-mono text-[var(--app-text-secondary)] whitespace-pre-wrap leading-relaxed">
                          {updateInfo.release_notes}
                        </pre>
                      </div>
                    )}

                    {updateInfo.release_url && (
                      <a
                        href={updateInfo.release_url}
                        target="_blank"
                        rel="noreferrer"
                        className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-[var(--app-accent)] hover:bg-[var(--app-accent-hover)] text-[var(--app-accent-foreground)] text-sm font-bold transition-colors cursor-pointer no-underline"
                      >
                        <ExternalLink className="w-3.5 h-3.5" />
                        前往下载
                      </a>
                    )}
                  </>
                ) : (
                  <>
                    <div className="flex items-start gap-2 text-xs text-red-400">
                      <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                      <div className="flex-1">
                        <div className="font-bold">检查更新失败</div>
                        {error && (
                          <div className="mt-1 text-[10px] text-red-300/80 leading-relaxed font-mono">
                            {error}
                          </div>
                        )}
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => void checkForUpdate({ force: true })}
                      className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] text-[var(--app-text-secondary)] hover:text-[var(--app-text)] text-sm font-bold transition-colors cursor-pointer"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      重试
                    </button>
                  </>
                )}
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}
