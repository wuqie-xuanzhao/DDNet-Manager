import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "framer-motion";
import { Gamepad } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type WheelEvent } from "react";
import logoMark from "./assets/logo.svg";
import { GAMES_DATA, GAME_ICON_MAP, buildGamesData, type LauncherGameId } from "./components/launcher/data";
import { ConfirmExitModal, PostDetailModal } from "./components/launcher/Dialogs";
import DownloadButton from "./components/launcher/DownloadButton";
import { InstallDialog } from "./components/launcher/InstallDialog";
import NewsCard from "./components/launcher/NewsCard";
import SocialSidebar from "./components/launcher/SocialSidebar";
import type { GameConfig, GameNewsItem, SocialLink } from "./components/launcher/types";
import VideoPlayer from "./components/launcher/VideoPlayer";
import WindowControls from "./components/launcher/WindowControls";
import { soundEngine } from "./components/launcher/audio";
import { SettingsDialog, type SettingsSectionId } from "./components/settings/SettingsDialog";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAutoUpdate } from "./hooks/useAutoUpdate";
import { useClientLauncher } from "./hooks/useClientLauncher";
import { useClientInstaller } from "./hooks/useClientInstaller";
import { useAppUpdater } from "./hooks/useAppUpdater";
import { useHotkey } from "./hooks/useHotkey";
import { isTauriRuntime, convertFileSrc, getClientCatalog } from "./lib/tauri";
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
  const [activeGameId, setActiveGameId] = useState<LauncherGameId>("ddnet");
  const [displayedGameId, setDisplayedGameId] = useState<LauncherGameId>("ddnet");
  const [isLibraryOpen, setIsLibraryOpen] = useState(false);
  const [hoveredGameId, setHoveredGameId] = useState<LauncherGameId | null>(null);
  const [hoveredCardIndex, setHoveredCardIndex] = useState<number | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [activeSettingsSection, setActiveSettingsSection] = useState<SettingsSectionId>("general");
  const [isExitAlertOpen, setIsExitAlertOpen] = useState(false);
  const [isPlayerOpen, setIsPlayerOpen] = useState(false);
  const [selectedPost, setSelectedPost] = useState<GameNewsItem | null>(null);
  const [isPostOpen, setIsPostOpen] = useState(false);
  const [isAudioOn, setIsAudioOn] = useState(false);

  const [customBgs, setCustomBgs] = useState<Record<string, { type: "default" | "image" | "video"; path: string }>>(() => {
    try {
      const saved = localStorage.getItem("ddnet_manager_custom_bgs");
      return saved ? JSON.parse(saved) : {};
    } catch {
      return {};
    }
  });
  const [themeMode, setThemeMode] = useState<"dark" | "light">(() => {
    return (localStorage.getItem("ddnet_manager_theme") as "dark" | "light") || "dark";
  });

  useEffect(() => {
    if (themeMode === "light") {
      document.documentElement.classList.add("theme-light");
    } else {
      document.documentElement.classList.remove("theme-light");
    }
  }, [themeMode]);

  const handleCustomBgChange = (gameId: string, type: "default" | "image" | "video", path: string) => {
    const nextBgs = {
      ...customBgs,
      [gameId]: { type, path }
    };
    setCustomBgs(nextBgs);
    localStorage.setItem("ddnet_manager_custom_bgs", JSON.stringify(nextBgs));
  };

  const handleThemeChange = (theme: "dark" | "light") => {
    setThemeMode(theme);
    localStorage.setItem("ddnet_manager_theme", theme);
  };

  const trackRef = useRef<HTMLDivElement | null>(null);
  const scrollTargetRef = useRef<number | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const enterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const leaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { appSettings, savedAppSettings, settingsState, settingsError, updateAndSave } = useAppSettings(tauriRuntime);
  const {
    errorMessage,
    handleBrowse,
    handlePrimaryAction,
    launcherState,
    selectedClient
  } = useClientLauncher({
    activeGameId,
    appSettings,
    localSmokeAutomation,
    onOpenUpdateView: () => {
      setIsSettingsOpen(true);
      setActiveSettingsSection("download");
    },
    tauriRuntime
  });
  useAutoUpdate({
    savedAppSettings,
    selectedClient,
    settingsState,
    tauriRuntime
  });

  // 主界面当前 tab 的安装/下载 hook。DownloadButton + InstallDialog 共享这一个实例。
  const installer = useClientInstaller({
    gameId: activeGameId,
    appSettings,
    tauriRuntime
  });

  // 启动器自身更新检查。启动后自动调一次（如果 allow_silent_update=true），
  // 右上角 AppUpdateButton 显示状态。
  const appUpdater = useAppUpdater({
    tauriRuntime,
    appSettings
  });

  // E1: 全局快捷键。Ctrl+, 打开设置（业界惯例，VS Code / GitHub Desktop 等都用此键）。
  // macOS 上 Cmd+, 也匹配（useHotkey ctrl 修饰符同时接受 metaKey）。
  // setIsSettingsOpen 来自 useState 是稳定引用，bindings 数组即使每次 render 重建，
  // effect 重新绑定 listener 的开销可接受。
  useHotkey([
    {
      key: ",",
      ctrl: true,
      handler: () => setIsSettingsOpen(true)
    }
  ]);

  const [gamesData, setGamesData] = useState(GAMES_DATA);

  // 启动时从 Rust catalog 拉业务数据，合并视觉资产生成动态 game tab。
  // 失败保留静态 fallback（GAMES_DATA），首屏不白屏。
  useEffect(() => {
    if (!tauriRuntime) {
      return;
    }
    let alive = true;
    void getClientCatalog()
      .then((catalog) => {
        if (!alive) return;
        const built = buildGamesData(catalog);
        if (built.length > 0) {
          setGamesData(built);
        }
      })
      .catch((err) => {
        console.error("Failed to load client catalog, using fallback:", err);
      });
    return () => {
      alive = false;
    };
  }, [tauriRuntime]);

  const activeGame = gamesData.find((game) => game.id === activeGameId) ?? gamesData[0];
  const displayedGame = gamesData.find((game) => game.id === displayedGameId) ?? activeGame;
  const currentBgConfig = useMemo(() => {
    const targetGameId = isLibraryOpen && hoveredGameId ? hoveredGameId : activeGameId;
    const defaultBg = (gamesData.find((game) => game.id === targetGameId) ?? activeGame).bgImage;
    const custom = customBgs[targetGameId];
    if (custom && custom.type !== "default" && custom.path) {
      return {
        type: custom.type,
        path: convertFileSrc(custom.path),
        fallbackUrl: defaultBg
      };
    }
    return {
      type: "default" as const,
      path: defaultBg,
      fallbackUrl: defaultBg
    };
  }, [isLibraryOpen, hoveredGameId, activeGameId, customBgs, activeGame, gamesData]);
  const repeatedGames = useMemo(() => gamesData, [gamesData]);

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
      // 线性滚动：库打开时滚回最左，让用户从头看起
      trackRef.current.scrollLeft = 0;
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

    // 线性滚动：到边界时 clamp，不再 wrap-around
    const maxScroll = container.scrollWidth - container.clientWidth;
    scrollTargetRef.current = Math.max(0, Math.min(maxScroll, scrollTargetRef.current));

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
    void currentWindow()?.hide();
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
              key={`${currentBgConfig.type}:${currentBgConfig.path}`}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.55 }}
              className="absolute inset-0"
            >
              {currentBgConfig.type === "video" ? (
                <video
                  autoPlay
                  loop
                  muted
                  playsInline
                  className="w-full h-full object-cover select-none pointer-events-none"
                  src={currentBgConfig.path}
                />
              ) : (
                <img
                  src={currentBgConfig.path}
                  alt={`${activeGame.name} 背景`}
                  className="w-full h-full object-cover select-none pointer-events-none"
                />
              )}
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
              <div className={`absolute w-[44px] h-[44px] rounded-[14px] border-[2px] transition-all duration-200 pointer-events-none z-0 ${isLibraryOpen ? "border-white/50 bg-[#1e2024]/40" : "border-white/10 group-hover:border-white/35"}`} />
              <motion.button
                id="btn-all-games-sidebar"
                type="button"
                aria-label="全部游戏"
                onClick={() => setIsLibraryOpen((value) => !value)}
                whileTap={{ scale: 0.94 }}
                className={`w-9 h-9 rounded-[11px] flex items-center justify-center cursor-pointer transition-all duration-150 relative z-10 focus:outline-none bg-[#24262b] border ${isLibraryOpen ? "border-white/20 bg-[#2b2d35]" : "border-white/5 hover:bg-[#2e3037] hover:border-white/15"}`}
              >
                <svg viewBox="0 0 24 24" className={`w-[21px] h-[21px] fill-current text-white/55 transition-all duration-200 ${isLibraryOpen ? "text-white opacity-100 font-bold" : "group-hover:text-white group-hover:opacity-100"}`}>
                  <rect x="3" y="3" width="7" height="7" rx="2" />
                  <rect x="3" y="14" width="7" height="7" rx="2" />
                  <rect x="14" y="14" width="7" height="7" rx="2" />
                  <path d="M17.5 2.5 Q17.5 7 22 7 Q17.5 7 17.5 11.5 Q17.5 7 13 7 Q17.5 7 17.5 2.5 Z" />
                </svg>
              </motion.button>
              <div className="absolute left-[56px] top-1/2 -translate-y-1/2 scale-90 translate-x-[-4px] opacity-0 group-hover:scale-100 group-hover:translate-x-0 group-hover:opacity-100 pointer-events-none transition-all duration-200 z-50 flex items-center origin-left">
                <div className="w-2.5 h-2.5 rotate-45 bg-[var(--app-tooltip-bg)] relative -mr-1 rounded-[1.5px]" />
                <div className="bg-[var(--app-tooltip-bg)] text-[var(--app-tooltip-fg)] text-[11px] font-bold px-3 py-1.5 rounded-lg shadow-2xl whitespace-nowrap tracking-wide">全部游戏</div>
              </div>
            </div>
          </div>
        </div>

        <div className="flex-1 h-full relative flex flex-col overflow-hidden">
          <div className="relative z-40 h-16 w-full flex items-center justify-end px-8 select-none pointer-events-auto shrink-0" data-tauri-drag-region>
            <WindowControls
              appUpdater={appUpdater}
              onOpenSettings={() => setIsSettingsOpen(true)}
              onCloseLauncher={() => {
                if (appSettings.close_behavior === "minimize_to_tray") {
                  void currentWindow()?.hide();
                } else if (appSettings.close_behavior === "exit_launcher") {
                  void currentWindow()?.close();
                } else {
                  setIsExitAlertOpen(true);
                }
              }}
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
                installer={installer}
                accentColor={activeGame.accentColor}
              />
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
                    <div ref={trackRef} onWheel={handleWheel} className="flex items-center space-x-4 overflow-x-auto py-2 px-12 scrollbar-none" style={{ scrollBehavior: "auto" }}>
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
        <InstallDialog
          installer={installer}
          displayName={activeGame.name}
          gameId={activeGame.id}
        />
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
        <SettingsDialog
          open={isSettingsOpen}
          activeSection={activeSettingsSection}
          onSectionChange={setActiveSettingsSection}
          onClose={() => setIsSettingsOpen(false)}
          tauriRuntime={tauriRuntime}
          launcherState={launcherState}
          clientPath={selectedClient?.install_dir ?? ""}
          selectedClientType={{ name: selectedClient?.display_name ?? "未设置" }}
          customBgs={customBgs}
          activeGameId={activeGameId}
          onCustomBgChange={handleCustomBgChange}
          themeMode={themeMode}
          onThemeChange={handleThemeChange}
          errorMessage={errorMessage}
          settings={appSettings}
          settingsState={settingsState}
          settingsError={settingsError}
          smokeAutomation={localSmokeAutomation}
          onUpdateSettings={updateAndSave}
          appUpdater={appUpdater}
          onClientPathChange={() => {}}
          onBrowse={handleBrowse}
          onValidate={handlePrimaryAction}
        />
      </motion.div>
    </div>
  );
}
