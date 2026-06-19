import { Download, Pause, RefreshCw, AlertCircle, Wrench, Loader2, ChevronDown } from 'lucide-react';
import { AnimatePresence, motion } from 'framer-motion';
import type { ClientInstallation } from '@/types';
import type { useClientInstaller, ClientInstallState } from '@/hooks/useClientInstaller';
import { usePopoverState } from '@/hooks/usePopoverState';

interface DownloadButtonProps {
  installer: ReturnType<typeof useClientInstaller>;
  accentColor: string;
}

/// 主按钮（圆角胶囊 + 圆形图标容器 + amber 配色）。
/// 保留原米哈游风格视觉，仅图标和文案随 state 切换。
function PrimaryButton(props: {
  onClick: () => void;
  disabled?: boolean;
  icon: React.ReactNode;
  label: string;
  width?: number;
  testId?: string;
}) {
  const widthClass = props.width ? `w-[${props.width}px]` : 'w-[196px]';
  return (
    <motion.button
      data-testid={props.testId}
      type="button"
      onClick={props.onClick}
      disabled={props.disabled}
      whileHover={{ scale: 1.03, y: -1 }}
      whileTap={{ scale: 0.97 }}
      className={`group/btn ${widthClass} h-[52px] rounded-full flex items-center justify-start pl-[11px] pr-[16px] transition-all duration-200 cursor-pointer select-none no-underline border-none bg-[var(--app-accent)] hover:bg-[#252932] text-[var(--app-accent-foreground)] hover:text-[var(--app-accent)] shadow-[0_4px_16px_rgba(254,211,48,0.25)] hover:shadow-[0_4px_16px_rgba(37,41,50,0.3)] focus:outline-none disabled:cursor-not-allowed disabled:opacity-50`}
    >
      <div className="w-8 h-8 rounded-full bg-[#121319] group-hover/btn:bg-[var(--app-accent)] flex items-center justify-center mr-2.5 shrink-0 relative transition-colors duration-200">
        {props.icon}
      </div>
      <span className="font-extrabold text-[16px] tracking-wide leading-none select-none no-underline flex-1 text-center pr-1.5">
        {props.label}
      </span>
    </motion.button>
  );
}

/// 主按钮下方的小链接（米哈游"已安装？定位游戏"风格）。
function SmallLink(props: { prefix?: string; text: string; onClick?: () => void }) {
  return (
    <div className="text-[12px] text-white/60 flex items-center space-x-1 font-sans justify-center mt-1">
      {props.prefix ? <span>{props.prefix}</span> : null}
      {props.onClick ? (
        <button
          type="button"
          onClick={props.onClick}
          className="text-[var(--app-accent)] hover:text-[var(--app-accent-hover)] font-bold cursor-pointer transition-colors focus:outline-none bg-transparent border-none p-0 inline-block ml-0.5"
        >
          {props.text}
        </button>
      ) : (
        <span className="text-white/45 font-mono ml-0.5">{props.text}</span>
      )}
    </div>
  );
}

/// 多副本切换器。仅 installer.clients.length > 1 时由父组件渲染。
/// 显示 "副本 1/2" + 下拉箭头，点击展开列表选择具体副本。
function InstanceSwitcher(props: {
  clients: ClientInstallation[];
  selectedId: string;
  currentIndex: number;
  onSelect: (id: string) => void;
}) {
  const { open, setOpen, backdropProps } = usePopoverState();
  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="inline-flex items-center gap-1 text-[11px] text-white/55 hover:text-white font-mono transition-colors cursor-pointer focus:outline-none bg-transparent border-none"
      >
        副本 {props.currentIndex}/{props.clients.length}
        <ChevronDown className={`w-3 h-3 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>
      <AnimatePresence>
        {open && (
          <>
            <div className="fixed inset-0 z-40" {...backdropProps} />
            <motion.div
              initial={{ opacity: 0, scale: 0.95, y: -6 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: -6 }}
              transition={{ duration: 0.15 }}
              className="absolute top-[120%] left-1/2 -translate-x-1/2 z-50 w-72 bg-[var(--app-surface)] border border-[var(--app-border)] rounded-lg shadow-[0_12px_40px_rgba(0,0,0,0.6)] overflow-hidden"
            >
              <div className="px-3 py-2 border-b border-[var(--app-border-subtle)] text-[10px] font-bold uppercase tracking-wider text-[var(--app-text-muted)]">
                选择客户端副本
              </div>
              <ul className="max-h-60 overflow-y-auto">
                {props.clients.map((c, idx) => {
                  const isSelected = c.id === props.selectedId;
                  return (
                    <li key={c.id}>
                      <button
                        type="button"
                        onClick={() => {
                          props.onSelect(c.id);
                          setOpen(false);
                        }}
                        className={`w-full text-left px-3 py-2 text-xs transition-colors cursor-pointer flex items-center justify-between gap-2 border-none ${
                          isSelected
                            ? 'bg-[var(--app-accent-subtle)] text-[var(--app-accent)] font-bold'
                            : 'bg-transparent text-[var(--app-text-secondary)] hover:bg-white/5'
                        }`}
                      >
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-mono">{c.install_dir}</div>
                          <div className="text-[10px] text-[var(--app-text-dim)] mt-0.5">
                            v{c.version ?? '未知'} · #{idx + 1}
                            {c.is_default && ' · 默认'}
                          </div>
                        </div>
                        {isSelected && <span className="text-[10px] shrink-0">✓</span>}
                      </button>
                    </li>
                  );
                })}
              </ul>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}

function getAccentBarColor(accentColor: string): string {
  switch (accentColor) {
    case 'indigo':
      return 'bg-indigo-500';
    case 'yellow':
      return 'bg-yellow-400';
    default:
      return 'bg-yellow-400';
  }
}

function DownloadArrowIcon() {
  return (
    <motion.svg
      viewBox="0 0 24 24"
      className="w-4 h-4 text-[var(--app-accent)] group-hover/btn:text-[#121319] transition-colors duration-200"
      fill="none"
      stroke="currentColor"
      strokeWidth="3.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      animate={{ y: [-2.5, 2.5, -2.5] }}
      transition={{ duration: 1.4, repeat: Infinity, ease: "easeInOut" }}
    >
      <line x1="12" y1="5" x2="12" y2="19" />
      <polyline points="19 12 12 19 5 12" />
    </motion.svg>
  );
}

function PlayIcon() {
  return (
    <svg viewBox="0 0 24 24" className="w-4 h-4 fill-current text-[var(--app-accent)] group-hover/btn:text-[#121319] transition-colors duration-200 ml-0.5">
      <path d="M8 5v14l11-7z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    </svg>
  );
}

function renderState(
  state: ClientInstallState,
  accentColor: string,
  installer: ReturnType<typeof useClientInstaller>
): React.ReactNode {
  switch (state.kind) {
    case 'loading':
      return (
        <div className="flex flex-col items-center space-y-2">
          <div className="w-[196px] h-[52px] rounded-full flex items-center justify-center bg-black/30 border border-white/5">
            <Loader2 className="w-5 h-5 text-white/50 animate-spin" />
          </div>
        </div>
      );

    case 'unknown':
    case 'not_installed':
      return (
        <motion.div
          key="uninstalled"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          className="flex flex-col items-center space-y-2"
        >
          <PrimaryButton
            testId="btn-install-game"
            onClick={installer.openInstallDialog}
            icon={<DownloadArrowIcon />}
            label="获取游戏"
          />
          <SmallLink
            prefix="已安装？"
            text={installer.scanning ? '扫描中…' : '定位游戏'}
            onClick={installer.scanning ? undefined : installer.triggerScan}
          />
        </motion.div>
      );

    case 'installed': {
      const smallLink = state.needsUpdate
        ? { text: `获取更新 v${state.latest ?? '?'} →`, onClick: installer.openUpdateDialog }
        : state.version
          ? { text: `v${state.version}`, onClick: undefined as (() => void) | undefined }
          : null;
      // 多副本切换器：仅 clients.length > 1 时显示，放在主按钮和小链接之间
      const showInstanceSwitcher = installer.clients.length > 1;
      const currentClientId = installer.client?.id ?? "";
      const currentIndex = installer.client
        ? installer.clients.findIndex((c) => c.id === currentClientId) + 1
        : 1;
      return (
        <motion.div
          key="installed"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          className="flex flex-col items-center space-y-2"
        >
          <PrimaryButton
            testId="btn-launch-game"
            width={172}
            onClick={installer.launchGame}
            icon={<PlayIcon />}
            label="开始游戏"
          />
          {showInstanceSwitcher && (
            <InstanceSwitcher
              clients={installer.clients}
              selectedId={installer.client?.id ?? ''}
              onSelect={installer.selectClient}
              currentIndex={currentIndex}
            />
          )}
          {smallLink ? <SmallLink text={smallLink.text} onClick={smallLink.onClick} /> : null}
        </motion.div>
      );
    }

    case 'broken':
      return (
        <motion.div
          key="broken"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          className="flex flex-col items-center space-y-2"
        >
          <PrimaryButton
            testId="btn-repair-game"
            onClick={installer.openUpdateDialog}
            icon={<Wrench className="w-4 h-4 text-[var(--app-accent)] group-hover/btn:text-[#121319] transition-colors duration-200" />}
            label="修复"
          />
          <SmallLink text="重新定位" onClick={installer.triggerScan} />
        </motion.div>
      );

    case 'downloading': {
      const percent = Math.min(100, Math.max(0, state.progress * 100));
      return (
        <motion.div
          key="downloading-state"
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="w-[240px] bg-black/60 border border-white/10 backdrop-blur-md rounded-2xl p-4 flex flex-col space-y-2.5 shadow-2xl z-30"
        >
          <div className="flex items-center justify-between">
            <div>
              <span className="text-white text-xs font-bold tracking-wide">
                {state.paused ? '暂停' : '下载中'}
              </span>
              <p className="text-[10px] text-gray-400 font-mono leading-none mt-0.5">
                {/* DownloadJob 当前无 rate 字段，speedBytesPerSec 是占位 0；
                    暂不显示具体速率，避免死值（review issue #3） */}
                {state.paused ? '已暂停' : '正在下载…'}
              </p>
            </div>
            <div className="flex space-x-1.5">
              {state.paused ? (
                <button
                  type="button"
                  title="继续下载"
                  className="w-6 h-6 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center text-white transition-colors cursor-pointer"
                >
                  <Download className="w-3 h-3" />
                </button>
              ) : (
                <button
                  type="button"
                  title="暂停下载"
                  className="w-6 h-6 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center text-white transition-colors cursor-pointer"
                >
                  <Pause className="w-3 h-3" />
                </button>
              )}
            </div>
          </div>
          <div className="relative w-full h-2 bg-white/10 rounded-full overflow-hidden">
            <div
              className={`h-full absolute left-0 top-0 rounded-full transition-all duration-300 ${getAccentBarColor(accentColor)}`}
              style={{ width: `${percent}%` }}
            />
          </div>
          <div className="flex items-center justify-between text-[9px] font-mono text-gray-400">
            <span>{percent.toFixed(1)}%</span>
          </div>
        </motion.div>
      );
    }

    case 'verifying':
      return (
        <motion.div
          key="verifying"
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="w-[240px] bg-black/60 border border-white/10 backdrop-blur-md rounded-2xl p-4 flex flex-col items-center justify-center space-y-3.5 shadow-2xl z-30"
        >
          <RefreshCw className="w-6 h-6 text-[var(--app-accent)] animate-spin" />
          <span className="text-white text-xs font-bold tracking-wide block">校验中</span>
        </motion.div>
      );

    case 'failed':
      return (
        <motion.div
          key="failed"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          className="flex flex-col items-center space-y-2"
        >
          <PrimaryButton
            testId="btn-retry-game"
            onClick={installer.openInstallDialog}
            icon={<AlertCircle className="w-4 h-4 text-[var(--app-accent)] group-hover/btn:text-[#121319] transition-colors duration-200" />}
            label="重试"
          />
          <div className="text-[11px] text-red-400/80 max-w-[200px] text-center leading-relaxed">
            {state.error}
          </div>
        </motion.div>
      );
  }
}

export default function DownloadButton({ installer, accentColor }: DownloadButtonProps) {
  return (
    <div className="flex flex-col items-center">
      <AnimatePresence mode="wait">{renderState(installer.state, accentColor, installer)}</AnimatePresence>
    </div>
  );
}
