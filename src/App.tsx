import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "framer-motion";
import { Gamepad, Loader2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type UIEvent, type WheelEvent } from "react";
import logoMark from "./assets/logo.svg";
import { GAMES_DATA, GAME_ICON_MAP, type LauncherGameId } from "./components/launcher/data";
import { ConfirmExitModal, PostDetailModal, SettingsModal } from "./components/launcher/Dialogs";
import DownloadButton from "./components/launcher/DownloadButton";
import NewsCard from "./components/launcher/NewsCard";
import SocialSidebar from "./components/launcher/SocialSidebar";
import type { GameConfig, GameNewsItem, SocialLink } from "./components/launcher/types";
import VideoPlayer from "./components/launcher/VideoPlayer";
import WindowControls from "./components/launcher/WindowControls";
import { soundEngine } from "./components/launcher/audio";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAutoUpdate } from "./hooks/useAutoUpdate";
import { useClientLauncher } from "./hooks/useClientLauncher";
import { isTauriRuntime } from "./lib/tauri";
import type { LocalSmokeAutomationConfig } from "./types";

function localSmokeEnvEnabled(value: string | undefined) {
  return value?.trim() === "1";
}

function resolveLocalSmokeAutomation(): LocalSmokeAutomationConfig | null {
  if (!localSmokeEnvEnabled(import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE)) {
    return null;
  }

  return {
    clientInstallDir: import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR?.trim() ?? "",
    manifestUrl: import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL?.trim() ?? "",
    closeWindowOnFinish: localSmokeEnvEnabled(import.meta.env.VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH)
  };
}

const localSmokeAutomation = resolveLocalSmokeAutomation();

const descriptions: Record<string, string> = {
  qmclient: "QmClient 是当前主线客户端。DDNet Manager 会围绕它管理默认启动项、版本检测、下载校验、安装事务和失败恢复记录。",
  ddnet: "DDNet 官方客户端保留原版 DDrace Network 体验。你可以通过扫描或手动定位，把它纳入统一启动与注册表管理。",
  "ddnet-steam": "Steam 版 DDNet 会从 steamapps/common/DDNet 识别安装位置，并和其他客户端一样参与默认客户端选择与运行检测。",
  "third-party": "第三方兼容客户端可通过手动路径加入管理。模型预留后续 catalog、更新源和安装历史扩展。"
};

const particles = Array.from({ length: 14 }, (_, index) => ({
  delay: -index * 1.37,
  duration: 16 + (index % 6) * 2.4,
  left: `${(index * 17) % 100}%`,
  opacity: 0.18 + (index % 5) * 0.07,
  size: 3 + (index % 4)
}));

function currentWindow() {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

function getAccentColor(gameId: string) {
  if (gameId === "qmclient") {
    return "rgba(254,211,48,0.75)";
  }
  if (gameId === "ddnet") {
    return "rgba(99,102,241,0.72)";
  }
  if (gameId === "ddnet-steam") {
    return "rgba(240,218,22,0.82)";
  }
  return "rgba(204,43,125,0.72)";
}

function GameLogo(props: { game: GameConfig; large?: boolean }) {
  return (
    <div className="flex flex-col text-left text-white leading-none">
      <span
        className={`${props.large ? "text-4xl sm:text-[46px]" : "text-[24px]"} font-black tracking-tight text-transparent bg-clip-text bg-gradient-to-r from-white via-white to-gray-300 drop-shadow-[0_4px_12px_rgba(255,255,255,0.08)] select-none`}
      >
        {props.game.logoText}
      </span>
      <span className={`${props.large ? "text-[11px]" : "text-[9px]"} uppercase tracking-[0.25em] text-[#fed330] font-black font-mono mt-1.5 select-none`}>
        {props.game.logoSubtext}
      </span>
    </div>
  );
}

export default function App() {
  const tauriRuntime = isTauriRuntime();
  const [activeGameId, setActiveGameId] = useState<LauncherGameId>("qmclient");
  const [displayedGameId, setDisplayedGameId] = useState<LauncherGameId>("qmclient");
  const [isLibraryOpen, setIsLibraryOpen] = useState(false);
  const [hoveredGameId, setHoveredGameId] = useState<LauncherGameId | null>(null);
  const [hoveredCardIndex, setHoveredCardIndex] = useState<number | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isExitAlertOpen, setIsExitAlertOpen] = useState(false);
  const [isPlayerOpen, setIsPlayerOpen] = useState(false);
  const [selectedPost, setSelectedPost] = useState<GameNewsItem | null>(null);
  const [isPostOpen, setIsPostOpen] = useState(false);
  const [isAudioOn, setIsAudioOn] = useState(false);
  const [speedLimit, setSpeedLimit] = useState(false);
  const [bgmVolume, setBgmVolume] = useState(65);
  const [showGameIcons, setShowGameIcons] = useState(true);
  const [isLaunching, setIsLaunching] = useState(false);
  const [launchProgress, setLaunchProgress] = useState(0);
  const [launchStatusText, setLaunchStatusText] = useState("");

  const trackRef = useRef<HTMLDivElement | null>(null);
  const scrollTargetRef = useRef<number | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const enterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const leaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { appSettings, savedAppSettings, settingsState } = useAppSettings(tauriRuntime);
  const {
    errorMessage,
    handleBrowse,
    handlePrimaryAction,
    launchReadiness,
    launcherState,
    selectedClient
  } = useClientLauncher({
    appSettings,
    localSmokeAutomation,
    onOpenUpdateView: () => undefined,
    tauriRuntime
  });
  useAutoUpdate({
    savedAppSettings,
    selectedClient,
    settingsState,
    tauriRuntime
  });

  const activeGame = GAMES_DATA.find((game) => game.id === activeGameId) ?? GAMES_DATA[0];
  const displayedGame = GAMES_DATA.find((game) => game.id === displayedGameId) ?? activeGame;
  const activeWallpaper = isLibraryOpen && hoveredGameId ? (GAMES_DATA.find((game) => game.id === hoveredGameId)?.bgImage ?? activeGame.bgImage) : activeGame.bgImage;
  const repeatedGames = useMemo(() => [...GAMES_DATA, ...GAMES_DATA, ...GAMES_DATA, ...GAMES_DATA, ...GAMES_DATA], []);
  const canLaunch = Boolean(launchReadiness?.can_launch) && launcherState === "ready";
  const primaryDisabled = !tauriRuntime || launcherState === "validating" || launcherState === "launching" || launcherState === "running";

  useEffect(() => {
    if (isAudioOn) {
      soundEngine.start(activeGameId);
      return;
    }

    soundEngine.stop();
  }, [activeGameId, isAudioOn]);

  useEffect(() => {
    return () => {
      if (enterTimerRef.current) {
        clearTimeout(enterTimerRef.current);
      }
      if (leaveTimerRef.current) {
        clearTimeout(leaveTimerRef.current);
      }
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!isLibraryOpen) {
      setHoveredCardIndex(null);
      setHoveredGameId(null);
      return;
    }

    setDisplayedGameId(activeGameId);
    scrollTargetRef.current = null;
    if (trackRef.current) {
      const itemWidth = 196;
      trackRef.current.scrollLeft = itemWidth * GAMES_DATA.length * 2;
    }
  }, [activeGameId, isLibraryOpen]);

  const handleCardMouseEnter = (gameId: string, index: number) => {
    if (enterTimerRef.current) {
      clearTimeout(enterTimerRef.current);
    }
    if (leaveTimerRef.current) {
      clearTimeout(leaveTimerRef.current);
    }

    setHoveredCardIndex(index);
    enterTimerRef.current = setTimeout(() => {
      setHoveredGameId(gameId as LauncherGameId);
      setDisplayedGameId(gameId as LauncherGameId);
    }, 240);
  };

  const handleCardMouseLeave = () => {
    if (enterTimerRef.current) {
      clearTimeout(enterTimerRef.current);
    }
    setHoveredCardIndex(null);
    leaveTimerRef.current = setTimeout(() => {
      setHoveredGameId(null);
      setDisplayedGameId(activeGameId);
    }, 220);
  };

  const handleWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (!trackRef.current) {
      return;
    }

    const container = trackRef.current;
    scrollTargetRef.current ??= container.scrollLeft;
    scrollTargetRef.current += event.deltaY * 1.1;

    const animateScroll = () => {
      if (!trackRef.current || scrollTargetRef.current === null) {
        return;
      }

      const current = trackRef.current.scrollLeft;
      const target = scrollTargetRef.current;
      const diff = target - current;

      if (Math.abs(diff) > 0.4) {
        trackRef.current.scrollLeft += diff * 0.12;
        animationFrameRef.current = requestAnimationFrame(animateScroll);
        return;
      }

      trackRef.current.scrollLeft = target;
      scrollTargetRef.current = null;
      animationFrameRef.current = null;
    };

    animationFrameRef.current ??= requestAnimationFrame(animateScroll);
  };

  const handleScroll = (event: UIEvent<HTMLDivElement>) => {
    const container = event.currentTarget;
    const itemWidth = 196;
    const singleSetWidth = itemWidth * GAMES_DATA.length;
    let delta = 0;

    if (container.scrollLeft >= singleSetWidth * 3) {
      delta = -singleSetWidth;
    } else if (container.scrollLeft <= singleSetWidth) {
      delta = singleSetWidth;
    }

    if (delta !== 0) {
      container.scrollLeft += delta;
      if (scrollTargetRef.current !== null) {
        scrollTargetRef.current += delta;
      }
    }
  };

  const handleLaunchGame = async () => {
    if (primaryDisabled) {
      return;
    }

    if (!canLaunch) {
      await handleBrowse();
      return;
    }

    setIsLaunching(true);
    setLaunchProgress(0);
    setLaunchStatusText("正在复检默认客户端...");
    const interval = setInterval(() => {
      setLaunchProgress((current) => {
        const next = Math.min(100, current + 8);
        if (next >= 32) {
          setLaunchStatusText("正在同步本地注册表与可执行文件...");
        }
        if (next >= 68) {
          setLaunchStatusText("正在拉起客户端进程...");
        }
        if (next >= 100) {
          clearInterval(interval);
          void handlePrimaryAction().finally(() => setTimeout(() => setIsLaunching(false), 500));
        }
        return next;
      });
    }, 120);
  };

  const handleSocialClick = (social: SocialLink) => {
    if (social.url.startsWith("http")) {
      window.open(social.url, "_blank", "noreferrer");
    }
  };

  const handleConfirmExit = () => {
    setIsExitAlertOpen(false);
    void currentWindow()?.close();
  };

  const handleMinimizeFromExit = () => {
    setIsExitAlertOpen(false);
    void currentWindow()?.minimize();
  };

  const selectGame = (gameId: string) => {
    const typedId = gameId as LauncherGameId;
    setActiveGameId(typedId);
    setDisplayedGameId(typedId);
    setIsLibraryOpen(false);
  };

  return (
    <div id="pc-desktop" className="fixed inset-0 h-screen w-screen overflow-hidden bg-[#111215] select-none font-sans">
      <motion.div id="ddnet-launcher" className="absolute inset-0 h-full w-full bg-[#111215] overflow-hidden flex flex-row select-none">
        <div className="absolute inset-0 z-0 select-none pointer-events-none overflow-hidden">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeWallpaper}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.55 }}
              className="absolute inset-0"
            >
              <img src={activeWallpaper} alt={`${activeGame.name} 背景`} className="w-full h-full object-cover select-none pointer-events-none" />
              <div className="absolute inset-0 bg-gradient-to-r from-black/85 via-black/30 to-black/10" />
              <div className="absolute inset-0 bg-gradient-to-t from-black/95 via-[#000000]/25 to-black/40" />
            </motion.div>
          </AnimatePresence>
        </div>

        <div className="absolute inset-0 pointer-events-none overflow-hidden z-1">
          {particles.map((particle, index) => {
            const bulletColor = getAccentColor(activeGame.id);
            return (
              <motion.div
                key={index}
                className="absolute rounded-full pointer-events-none"
                style={{
                  width: particle.size,
                  height: particle.size,
                  left: particle.left,
                  bottom: "-20px",
                  opacity: particle.opacity,
                  background: `radial-gradient(circle, ${bulletColor} 0%, rgba(255,255,255,0) 100%)`,
                  boxShadow: `0 0 ${particle.size * 1.5}px ${bulletColor}`
                }}
                animate={{ y: ["0vh", "-110vh"], x: ["0px", `${Math.sin(index) * 35 + 15}px`, `${Math.sin(index) * -35 - 15}px`, "0px"] }}
                transition={{ duration: particle.duration, repeat: Infinity, ease: "linear", delay: particle.delay }}
              />
            );
          })}
        </div>

        <div className="absolute inset-0 z-0 pointer-events-none overflow-hidden">
          <div className="absolute top-10 right-20 w-[350px] h-[350px] bg-blue-500/10 blur-[130px] rounded-full" />
          <div className="absolute bottom-20 left-10 w-[250px] h-[250px] bg-purple-500/10 blur-[110px] rounded-full" />
        </div>

        <div className="w-[58px] bg-[#111215]/12 backdrop-blur-lg border-r border-white/5 flex flex-col items-center py-6 shrink-0 z-40 relative select-none" data-tauri-drag-region>
          <motion.button
            type="button"
            whileHover={{ scale: 1.05 }}
            onClick={() => setIsSettingsOpen(true)}
            aria-label="DDNet Manager 首页"
            className="w-10 h-10 flex items-center justify-center cursor-pointer shrink-0"
          >
            <img src={logoMark} alt="" className="w-[34px] h-[34px] brightness-0 invert opacity-95" />
          </motion.button>

          <div className="flex-1" />
          <div className="w-[20px] h-[1.5px] bg-white/10 mb-3.5 shrink-0" />

          <div className="flex flex-col items-center w-full mb-2 select-none font-sans">
            <div className="relative group flex items-center justify-center w-full">
              <div className={`absolute w-[44px] h-[44px] rounded-[13px] border-[2px] transition-all duration-200 pointer-events-none z-0 ${isLibraryOpen ? "border-black/65 opacity-100" : "border-black/35 opacity-0 group-hover:opacity-100"}`} />
              <motion.button
                id="btn-all-games-sidebar"
                type="button"
                aria-label="全部游戏"
                onClick={() => setIsLibraryOpen((value) => !value)}
                whileTap={{ scale: 0.94 }}
                className="w-9 h-9 rounded-[10px] flex items-center justify-center cursor-pointer transition-all duration-150 relative z-10 focus:outline-none bg-[#28292e] border-transparent"
              >
                <svg viewBox="0 0 24 24" className={`w-[21px] h-[21px] fill-current text-white/55 transition-all duration-200 ${isLibraryOpen ? "text-white opacity-100 font-bold" : "group-hover:text-white group-hover:opacity-100"}`}>
                  <rect x="3" y="3" width="7" height="7" rx="2" />
                  <rect x="3" y="14" width="7" height="7" rx="2" />
                  <rect x="14" y="14" width="7" height="7" rx="2" />
                  <path d="M17.5 2.5 Q17.5 7 22 7 Q17.5 7 17.5 11.5 Q17.5 7 13 7 Q17.5 7 17.5 2.5 Z" />
                </svg>
              </motion.button>
              <div className="absolute left-[56px] top-1/2 -translate-y-1/2 scale-90 translate-x-[-4px] opacity-0 group-hover:scale-100 group-hover:translate-x-0 group-hover:opacity-100 pointer-events-none transition-all duration-200 z-50 flex items-center origin-left">
                <div className="w-2.5 h-2.5 rotate-45 bg-[#e3e5e9] relative -mr-1 rounded-[1.5px]" />
                <div className="bg-[#e3e5e9] text-[#121319] text-[11px] font-bold px-3 py-1.5 rounded-lg shadow-2xl whitespace-nowrap tracking-wide">全部游戏</div>
              </div>
            </div>
          </div>
        </div>

        <div className="flex-1 h-full relative flex flex-col overflow-hidden">
          <div className="relative z-40 h-16 w-full flex items-center justify-end px-8 select-none pointer-events-auto shrink-0" data-tauri-drag-region>
            <WindowControls
              onOpenSettings={() => setIsSettingsOpen(true)}
              onCloseLauncher={() => setIsExitAlertOpen(true)}
              onMinimize={() => void currentWindow()?.minimize()}
              isAudioOn={isAudioOn}
              onToggleAudio={() => setIsAudioOn((value) => !value)}
            />
          </div>

          <div className="absolute inset-0 z-30 pointer-events-none">
            <motion.div
              id="dashboard-intro-block"
              animate={{ opacity: isLibraryOpen ? 0 : 1, y: isLibraryOpen ? -50 : 0 }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? "none" : "auto" }}
              className="absolute top-[54px] left-10 z-40 flex flex-col space-y-5 select-none text-left max-w-[470px]"
            >
              <motion.div key={`logo-${activeGame.id}`} initial={{ opacity: 0, x: -10 }} animate={{ opacity: 1, x: 0 }} transition={{ duration: 0.4 }} className="flex items-center space-x-3 text-white mb-1.5">
                <GameLogo game={activeGame} />
              </motion.div>

              <motion.div key={`banner-${activeGame.id}`} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.5, delay: 0.1 }} className="space-y-2.5">
                <div className="flex items-center">
                  <div className="px-2.5 py-[3px] text-[11px] font-bold tracking-wide uppercase text-[#121319] flex items-center shadow-md select-none rounded-[3px] leading-none bg-[#fed330]">
                    {activeGame.bannerCategory}
                  </div>
                  <div className="flex flex-col space-y-[2px] ml-2">
                    <div className="flex space-x-[2px]">
                      <div className="w-[4.5px] h-[4.5px] rounded-[1px] bg-[#fed330]" />
                      <div className="w-[4.5px] h-[4.5px] bg-transparent" />
                      <div className="w-[4.5px] h-[4.5px] rounded-[1px] bg-[#fed330]" />
                    </div>
                    <div className="flex space-x-[2px]">
                      <div className="w-[4.5px] h-[4.5px] bg-transparent" />
                      <div className="w-[4.5px] h-[4.5px] rounded-[1px] bg-[#fed330]" />
                      <div className="w-[4.5px] h-[4.5px] bg-transparent" />
                    </div>
                  </div>
                </div>
                <h1
                  className="text-4xl sm:text-[50px] font-black tracking-wide leading-[0.98] select-none whitespace-nowrap"
                  style={{
                    background: "linear-gradient(to bottom, #ffffff 15%, #ffffff 50%, #f1e2c3 100%)",
                    WebkitBackgroundClip: "text",
                    WebkitTextFillColor: "transparent",
                    filter: "drop-shadow(0 4px 10px rgba(0,0,0,0.92)) drop-shadow(0 1px 2px rgba(0,0,0,1))"
                  }}
                >
                  {activeGame.bannerTitle}
                </h1>
                <p className="text-[13.5px] sm:text-[14.5px] font-bold text-[#ebe4d0] tracking-wide select-none [text-shadow:0_2px_4px_rgba(0,0,0,0.95),0_1px_1px_rgba(0,0,0,1)]">
                  {activeGame.bannerSubtitle}
                </p>
              </motion.div>

              <div className="pt-2">
                <motion.button
                  type="button"
                  whileHover={{ scale: 1.04, y: -1 }}
                  whileTap={{ scale: 0.97 }}
                  onClick={() => setIsPlayerOpen(true)}
                  className="bg-white hover:bg-neutral-100 text-[#121319] hover:text-black font-extrabold text-[13.5px] px-[28px] py-[10.5px] rounded-full shadow-[0_8px_20px_rgba(255,255,255,0.18)] transition-all cursor-pointer border-none flex items-center justify-center space-x-1.5 focus:outline-none"
                >
                  <span>{activeGame.bannerButtonText}</span>
                </motion.button>
              </div>
            </motion.div>

            <motion.div
              id="dashboard-news-block"
              animate={{ opacity: isLibraryOpen ? 0 : 1, y: isLibraryOpen ? 280 : 0 }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? "none" : "auto" }}
              className="absolute bottom-[18px] left-8 z-30 select-none flex flex-col space-y-4"
            >
              <NewsCard
                pvCardImage={activeGame.pvCardImage}
                pvTitle={activeGame.pvTitle}
                news={activeGame.news}
                accentColor={activeGame.accentColor}
                onOpenPV={() => setIsPlayerOpen(true)}
                onSelectNews={(item) => {
                  setSelectedPost(item);
                  setIsPostOpen(true);
                }}
              />
            </motion.div>

            <motion.div
              id="float-social-sidebar"
              animate={{ opacity: isLibraryOpen ? 0 : 1, scale: isLibraryOpen ? 0.85 : 1, x: isLibraryOpen ? 30 : 0 }}
              transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? "none" : "auto" }}
              className="absolute right-[14px] top-[41%] -translate-y-1/2 z-40 select-none"
            >
              <SocialSidebar socials={activeGame.socials} onSocialClick={handleSocialClick} />
            </motion.div>

            <motion.div
              id="dashboard-download-block"
              animate={{ opacity: isLibraryOpen ? 0 : 1, y: isLibraryOpen ? 280 : 0 }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? "none" : "auto" }}
              className="absolute bottom-[28px] right-8 z-35 select-none flex flex-col items-center font-sans"
            >
              <DownloadButton
                gameId={activeGame.id}
                sizeGB={activeGame.sizeGB}
                speedMB={activeGame.installSpeedMB}
                accentColor={activeGame.accentColor}
                onLaunchGame={() => void handleLaunchGame()}
                onLocateGame={() => void handleBrowse()}
                canLaunch={canLaunch}
                disabled={primaryDisabled}
              />
              {errorMessage ? <div className="mt-2 max-w-[280px] rounded-lg bg-red-500/12 px-3 py-2 text-center text-[11px] font-semibold text-red-200">{errorMessage}</div> : null}
            </motion.div>

            {isLibraryOpen ? (
              <>
                <motion.div
                  id="library-intro-block"
                  initial={{ opacity: 0, y: -50 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -50 }}
                  transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
                  className="absolute top-20 left-10 z-40 flex flex-col select-none text-left"
                >
                  <AnimatePresence mode="wait">
                    <motion.div key={`lib-logo-${displayedGameId}`} initial={{ opacity: 0, y: 15 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -15 }} transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}>
                      <GameLogo game={displayedGame} large />
                    </motion.div>
                  </AnimatePresence>
                </motion.div>

                <motion.div
                  id="library-description-block"
                  initial={{ opacity: 0, y: 40 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 40 }}
                  transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
                  className="absolute bottom-[170px] left-10 z-40 flex flex-col select-none text-left max-w-[650px]"
                >
                  <AnimatePresence mode="wait">
                    <motion.div key={`lib-desc-${displayedGameId}`} initial={{ opacity: 0, y: 15 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -15 }} transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}>
                      <p className="text-[15px] font-medium text-gray-200 leading-relaxed drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)] font-sans antialiased">
                        {descriptions[displayedGameId]}
                      </p>
                    </motion.div>
                  </AnimatePresence>
                </motion.div>

                <motion.div
                  id="library-carousel-block"
                  initial={{ opacity: 0, y: 280 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 280 }}
                  transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
                  className="absolute bottom-6 left-8 right-8 z-40 select-none pointer-events-auto"
                >
                  <div className="relative w-full overflow-hidden">
                    <div className="absolute left-0 top-0 bottom-0 w-16 bg-gradient-to-r from-[#111215] to-transparent pointer-events-none z-10" />
                    <div className="absolute right-0 top-0 bottom-0 w-16 bg-gradient-to-l from-[#111215] to-transparent pointer-events-none z-10" />
                    <div ref={trackRef} onWheel={handleWheel} onScroll={handleScroll} className="flex items-center space-x-4 overflow-x-auto py-2 px-12 scrollbar-none" style={{ scrollBehavior: "auto" }}>
                      {repeatedGames.map((game, index) => {
                        const isSelected = game.id === activeGameId;
                        const isCardHovered = index === hoveredCardIndex;
                        return (
                          <div key={`card-col-${game.id}-${index}`} className="flex flex-col items-center space-y-2.5 shrink-0">
                            <motion.button
                              id={`card-selector-${game.id}-${index}`}
                              type="button"
                              aria-label={`选择 ${game.name}`}
                              onMouseEnter={() => handleCardMouseEnter(game.id, index)}
                              onMouseLeave={handleCardMouseLeave}
                              onClick={() => selectGame(game.id)}
                              whileHover={{ y: -6, scale: 1.02 }}
                              whileTap={{ scale: 0.98 }}
                              className={`relative w-[180px] h-[95px] rounded-xl overflow-hidden cursor-pointer shrink-0 transition-[border-color,opacity,box-shadow] duration-200 border bg-transparent p-0 text-left focus:outline-none ${
                                isSelected
                                  ? "border-[#ffeb3b] shadow-[0_0_20px_rgba(255,235,59,0.35)] ring-1 ring-[#ffeb3b]/40 opacity-100"
                                  : isCardHovered
                                    ? "border-[#ffeb3b] shadow-[0_0_15px_rgba(255,235,59,0.25)] opacity-100"
                                    : "border-white/10 opacity-70"
                              }`}
                            >
                              <img src={game.pvCardImage} alt={game.name} className="w-full h-full object-cover" />
                              <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/20 to-transparent" />
                              {isSelected ? <div className="absolute top-1.5 right-1.5 px-2 py-0.5 rounded bg-yellow-400 text-black text-[9px] font-black uppercase tracking-wider block z-10">使用中</div> : null}
                              <AnimatePresence>
                                {isCardHovered ? (
                                  <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.16, ease: "easeOut" }} className="absolute inset-0 bg-black/75 flex items-center justify-center pointer-events-none z-20">
                                    <motion.div initial={{ scale: 0.94, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} exit={{ scale: 0.94, opacity: 0 }} transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }} className="flex items-center justify-center space-x-2">
                                      <div className="w-7 h-7 rounded-[8px] border border-white/10 bg-white/10 flex items-center justify-center overflow-hidden shrink-0 relative">
                                        <img src={GAME_ICON_MAP[game.id as LauncherGameId]} alt={game.name} className="w-full h-full object-cover absolute inset-0 z-10" onError={(event) => { event.currentTarget.style.opacity = "0"; }} />
                                        <div className="absolute inset-0 flex items-center justify-center text-white/40 z-0">
                                          <Gamepad className="w-3.5 h-3.5" />
                                        </div>
                                      </div>
                                      <span className="text-[#ffeb3b] text-[13px] font-bold tracking-wide">查看详情</span>
                                    </motion.div>
                                  </motion.div>
                                ) : null}
                              </AnimatePresence>
                            </motion.button>
                            <div className="text-center w-full">
                              <span className={`text-[12px] font-bold block tracking-wide select-none transition-colors duration-200 ${isSelected ? "text-[#ffeb3b] font-extrabold" : isCardHovered ? "text-white" : "text-[#9fa2b4]"}`}>
                                {game.name}
                              </span>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </motion.div>
              </>
            ) : null}
          </div>
        </div>

        <VideoPlayer isOpen={isPlayerOpen} onClose={() => setIsPlayerOpen(false)} title={activeGame.pvTitle} posterImage={activeGame.pvCardImage} />
        <PostDetailModal
          isOpen={isPostOpen}
          onClose={() => {
            setIsPostOpen(false);
            setSelectedPost(null);
          }}
          post={selectedPost}
          gameName={activeGame.name}
        />
        <ConfirmExitModal
          isOpen={isExitAlertOpen}
          onCancel={() => setIsExitAlertOpen(false)}
          onConfirmExit={handleConfirmExit}
          onMinimize={handleMinimizeFromExit}
        />
        <SettingsModal
          isOpen={isSettingsOpen}
          onClose={() => setIsSettingsOpen(false)}
          speedLimit={speedLimit}
          setSpeedLimit={setSpeedLimit}
          bgmVolume={bgmVolume}
          setBgmVolume={setBgmVolume}
          isAudioOn={isAudioOn}
          setIsAudioOn={setIsAudioOn}
          showGameIcons={showGameIcons}
          setShowGameIcons={setShowGameIcons}
        />

        <AnimatePresence>
          {isLaunching ? (
            <motion.div id="game-launch-curtain" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="absolute inset-0 bg-[#000] z-50 flex flex-col items-center justify-center p-6">
              <motion.div initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} className="flex flex-col items-center space-y-7 max-w-md text-center">
                <Loader2 className="w-10 h-10 text-yellow-400 animate-spin opacity-80" />
                <div className="space-y-2">
                  <h2 className="text-3xl font-extrabold tracking-widest text-[#fff]">{activeGame.name}</h2>
                  <p className="text-[11px] uppercase tracking-[0.25em] text-gray-500 font-mono">{activeGame.enName}</p>
                </div>
                <div className="w-[280px] h-1.5 bg-neutral-900 rounded-full overflow-hidden border border-white/5 relative">
                  <motion.div className="h-full bg-gradient-to-r from-yellow-400 via-amber-300 to-yellow-500 rounded-full" style={{ width: `${launchProgress}%` }} transition={{ ease: "easeOut" }} />
                </div>
                <span className="text-[13px] text-gray-400 block h-6 font-medium animate-pulse">{launchStatusText}</span>
                <span className="text-[11px] text-gray-600 block mt-4 font-mono">启动进度：{launchProgress}% · 加载中...</span>
              </motion.div>
            </motion.div>
          ) : null}
        </AnimatePresence>

        <div className="pointer-events-none absolute bottom-3 left-20 z-40 text-[10px] font-mono text-white/35">
          {launchReadiness?.user_message ?? "正在读取默认客户端状态"}
        </div>
      </motion.div>
    </div>
  );
}
