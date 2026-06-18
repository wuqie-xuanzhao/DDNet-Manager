import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Download, Loader2, AlertCircle, X, ExternalLink, RefreshCw, CheckCircle2 } from "lucide-react";
import type { ReactNode } from "react";
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

/// 通用右上角控制按钮：motion.button + 白底 tooltip 结构。
/// WindowControls 5 个按钮（声音/设置/最小化/关闭/更新）共用此结构，
/// review issue #16 抽公共组件消除 60+ 行 DOM 重复。
function ControlIconButton(props: {
  id: string;
  label: string;
  onClick: () => void;
  children: ReactNode;
  /// hover 时的文字色（默认 hover:text-white；声音按钮用 hover:text-[#fed330]，关闭按钮 hover:text-red-400）
  hoverColor?: string;
  /// 额外的按钮 className（如 ring、bg）
  buttonClassName?: string;
}) {
  return (
    <div className="relative group flex flex-col items-center">
      <motion.button
        id={props.id}
        type="button"
        aria-label={props.label}
        onClick={props.onClick}
        whileTap={{ scale: 0.92 }}
        className={`text-[#cccccc] ${props.hoverColor ?? "hover:text-white"} hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md ${props.buttonClassName ?? ""}`}
      >
        {props.children}
      </motion.button>
      <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
        <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
          <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
          <span className="relative z-10">{props.label}</span>
        </div>
      </div>
    </div>
  );
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

      {/* Sound Controller */}
      <ControlIconButton
        id="btn-bgm-sound-controller-top"
        label={isAudioOn ? "静音背景乐" : "播放背景乐"}
        onClick={onToggleAudio}
        hoverColor="hover:text-[#fed330]"
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
      </ControlIconButton>

      {/* Settings */}
      <ControlIconButton
        id="btn-settings"
        label="设置"
        onClick={onOpenSettings}
      >
        <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 2L20.66 7V17L12 22L3.34 17V7L12 2Z" />
          <circle cx="12" cy="12" r="3.2" />
        </svg>
      </ControlIconButton>

      {/* Minimize */}
      <ControlIconButton
        id="btn-minimize"
        label="最小化"
        onClick={onMinimize}
      >
        <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.8" strokeLinecap="round">
          <line x1="4" y1="12" x2="20" y2="12" />
        </svg>
      </ControlIconButton>

      {/* Close */}
      <ControlIconButton
        id="btn-close"
        label="关闭"
        onClick={onCloseLauncher}
        hoverColor="hover:text-red-400"
      >
        <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </ControlIconButton>
    </div>
  );
}

/// 启动器更新检查按钮。视觉与其他 4 个控制按钮一致（w-8 h-8 rounded-md），
/// 多了"发现新版本"时的红点 + 点击展开卡片显示版本信息 / 下载进度 / 安装状态。
function UpdateControlButton({ updater }: { updater: ReturnType<typeof useAppUpdater> }) {
  const {
    state,
    updateInfo,
    error,
    installError,
    progress,
    checkForUpdate,
    downloadAndInstall,
    cancelDownload,
    restartNow
  } = updater;
  const [open, setOpen] = useState(false);

  // 进入 downloading / installing / ready-to-restart 时自动展开 popup（用户点了"立即更新"后）
  // 显示进度反馈。退出这些状态时（cancel / failed）不强制收起，让用户看清楚结果。
  useEffect(() => {
    if (state === "downloading" || state === "installing" || state === "ready-to-restart") {
      setOpen(true);
    }
  }, [state]);

  if (!updater.visible) return null;

  const tooltipText = (() => {
    switch (state) {
      case "checking": return "检查更新中…";
      case "up-to-date": return "启动器已是最新";
      case "has-update": return `发现新版本 v${updateInfo?.latest_version ?? "?"}`;
      case "downloading":
        return progress && progress.ratio > 0
          ? `下载中 ${Math.round(progress.ratio * 100)}%`
          : "下载更新中…";
      case "installing": return "正在安装更新…";
      case "ready-to-restart": return "更新完成，等待重启";
      case "failed": return installError ? "安装失败" : "检查更新失败";
      default: return "检查启动器更新";
    }
  })();

  const iconColor = state === "has-update" || state === "ready-to-restart"
    ? "text-emerald-400"
    : state === "downloading" || state === "installing"
      ? "text-[var(--app-accent)]"
      : "text-[#cccccc]";
  const hoverColor = state === "has-update" || state === "ready-to-restart"
    ? "hover:text-emerald-300"
    : "hover:text-white";

  const handleClick = () => {
    if (state === "has-update") {
      // has-update：直接下载（不弹确认 popup）。downloadAndInstall 内部会触发 state 切换，
      // 配合上面的 useEffect 自动展开进度 popup。
      void downloadAndInstall();
    } else if (state === "ready-to-restart") {
      void restartNow();
    } else if (
      state === "downloading" ||
      state === "installing" ||
      state === "failed"
    ) {
      // 这些状态点击只切换 popup 显隐（查看进度 / 错误），不重新触发动作
      setOpen((v) => !v);
    } else {
      // idle / up-to-date / checking
      void checkForUpdate({ force: true });
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
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
        {state === "checking" || state === "installing" ? (
          <Loader2 className="w-[18px] h-[18px] animate-spin" />
        ) : state === "failed" ? (
          <AlertCircle className="w-[18px] h-[18px] text-red-400" />
        ) : state === "ready-to-restart" ? (
          <CheckCircle2 className="w-[18px] h-[18px]" />
        ) : state === "downloading" ? (
          <Download className="w-[18px] h-[18px] animate-pulse" />
        ) : (
          <>
            <Download className="w-[18px] h-[18px]" />
            {state === "has-update" && (
              <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            )}
          </>
        )}
      </motion.button>

      {/* 与其他 4 个按钮一致的白底 tooltip */}
      <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
        <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
          <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
          <span className="relative z-10">{tooltipText}</span>
        </div>
      </div>

      {/* 点击或自动展开的卡片：has-update / downloading / installing / ready-to-restart / failed */}
      <AnimatePresence>
        {open && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
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
                {state === "has-update" && updateInfo && (
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

                    <button
                      type="button"
                      onClick={() => void downloadAndInstall()}
                      className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-[var(--app-accent)] hover:bg-[var(--app-accent-hover)] text-[var(--app-accent-foreground)] text-sm font-bold transition-colors cursor-pointer"
                    >
                      <Download className="w-3.5 h-3.5" />
                      立即更新
                    </button>

                    {updateInfo.release_url && (
                      <a
                        href={updateInfo.release_url}
                        target="_blank"
                        rel="noreferrer"
                        className="block text-center text-[10px] text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] underline-offset-2 hover:underline transition-colors no-underline"
                      >
                        updater 失败？打开 Release 页手动下载
                      </a>
                    )}
                  </>
                )}

                {state === "downloading" && (
                  <>
                    <div className="flex items-center gap-2">
                      <Download className="w-4 h-4 text-[var(--app-accent)] animate-pulse" />
                      <span className="text-sm font-bold text-[var(--app-text)]">
                        正在下载 v{updateInfo?.latest_version ?? "新版本"}
                      </span>
                    </div>
                    <div className="space-y-1.5">
                      <div className="h-1.5 rounded-full bg-[var(--app-sunken)] overflow-hidden">
                        <div
                          className="h-full bg-[var(--app-accent)] transition-[width] duration-150 ease-out"
                          style={{
                            width: `${Math.max(2, Math.round((progress?.ratio ?? 0) * 100))}%`
                          }}
                        />
                      </div>
                      <div className="flex items-center justify-between text-[10px] text-[var(--app-text-muted)] font-mono">
                        <span>
                          {progress && progress.total > 0
                            ? `${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}`
                            : `${formatBytes(progress?.downloaded ?? 0)}`}
                        </span>
                        <span>{Math.round((progress?.ratio ?? 0) * 100)}%</span>
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={cancelDownload}
                      className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] text-[var(--app-text-secondary)] hover:text-[var(--app-text)] text-sm font-bold transition-colors cursor-pointer"
                    >
                      取消
                    </button>
                  </>
                )}

                {state === "installing" && (
                  <>
                    <div className="flex items-center gap-2">
                      <Loader2 className="w-4 h-4 animate-spin text-[var(--app-accent)]" />
                      <span className="text-sm font-bold text-[var(--app-text)]">
                        正在安装…
                      </span>
                    </div>
                    <div className="text-[11px] text-[var(--app-text-muted)] leading-relaxed">
                      安装器已启动，等待其完成。期间启动器可能短暂无响应。
                    </div>
                  </>
                )}

                {state === "ready-to-restart" && (
                  <>
                    <div className="flex items-center gap-2">
                      <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                      <span className="text-sm font-bold text-[var(--app-text)]">
                        安装完成
                      </span>
                    </div>
                    <div className="text-[11px] text-[var(--app-text-muted)] leading-relaxed">
                      点击下方按钮重启启动器，新版本立即生效。
                    </div>
                    <button
                      type="button"
                      onClick={() => void restartNow()}
                      className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-black text-sm font-bold transition-colors cursor-pointer"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      重启启动器
                    </button>
                    <button
                      type="button"
                      onClick={() => setOpen(false)}
                      className="block w-full text-center text-[10px] text-[var(--app-text-muted)] hover:text-[var(--app-text-secondary)] transition-colors cursor-pointer"
                    >
                      稍后手动重启
                    </button>
                  </>
                )}

                {state === "failed" && (
                  <>
                    <div className="flex items-start gap-2 text-xs text-red-400">
                      <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                      <div className="flex-1">
                        <div className="font-bold">
                          {installError ? "安装失败" : "检查更新失败"}
                        </div>
                        {(installError || error) && (
                          <div className="mt-1 text-[10px] text-red-300/80 leading-relaxed font-mono break-all">
                            {installError || error}
                          </div>
                        )}
                      </div>
                    </div>
                    {installError && updateInfo?.release_url ? (
                      // 安装失败时优先提供 fallback 链接
                      <a
                        href={updateInfo.release_url}
                        target="_blank"
                        rel="noreferrer"
                        className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-[var(--app-accent)] hover:bg-[var(--app-accent-hover)] text-[var(--app-accent-foreground)] text-sm font-bold transition-colors cursor-pointer no-underline"
                      >
                        <ExternalLink className="w-3.5 h-3.5" />
                        打开 Release 页手动下载
                      </a>
                    ) : (
                      <button
                        type="button"
                        onClick={() => void checkForUpdate({ force: true })}
                        className="flex items-center justify-center gap-1.5 w-full h-9 rounded-lg bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] text-[var(--app-text-secondary)] hover:text-[var(--app-text)] text-sm font-bold transition-colors cursor-pointer"
                      >
                        <RefreshCw className="w-3.5 h-3.5" />
                        重试检查
                      </button>
                    )}
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
