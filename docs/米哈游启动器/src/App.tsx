import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { 
  Grid2X2, 
  HelpCircle, 
  VolumeX, 
  Volume2, 
  Info, 
  Loader2, 
  Sparkles,
  Gamepad
} from 'lucide-react';

import { GAMES_DATA } from './data';
import { GameConfig, GameNewsItem, SocialLink } from './types';
import { soundEngine } from './utils/audio';

// Subcomponents
import WindowControls from './components/WindowControls';
import SocialSidebar from './components/SocialSidebar';
import NewsCard from './components/NewsCard';
import DownloadButton from './components/DownloadButton';
import VideoPlayer from './components/VideoPlayer';
import { PostDetailModal, SettingsModal, ConfirmExitModal } from './components/Dialogs';

const GAME_ICON_MAP: Record<string, string> = {
  'genshin-impact': 'https://img-os-static.hoyolab.com/game_record/ys/icon.png',
  'star-rail': 'https://img-os-static.hoyolab.com/game_record/hkrpg/icon.png',
  'zenless-zone-zero': 'https://img-os-static.hoyolab.com/game_record/nap/icon.png',
  'honkai-3rd': 'https://img-os-static.hoyolab.com/game_record/bh3/icon.png',
};

export default function App() {
  // Current Selected Game State (Default: Star Rail)
  const [activeGameId, setActiveGameId] = useState<string>('star-rail');
  const activeGame = GAMES_DATA.find(g => g.id === activeGameId) || GAMES_DATA[0];

  // UI Window Mode & Sizing States
  const [isFullscreen, setIsFullscreen] = useState<boolean>(false);
  const [isLibraryOpen, setIsLibraryOpen] = useState<boolean>(false);
  const [hoveredGameId, setHoveredGameId] = useState<string | null>(null);
  const [hoveredCardIndex, setHoveredCardIndex] = useState<number | null>(null);
  const [displayedGameId, setDisplayedGameId] = useState<string>('star-rail');

  // Preload all hover game icons for instant display with zero layout-pop-in
  useEffect(() => {
    Object.values(GAME_ICON_MAP).forEach((url) => {
      const img = new Image();
      img.src = url;
    });
  }, []);

  const getGameDescription = (id: string) => {
    switch (id) {
      case 'star-rail':
        return '搭乘星穹列车，探寻无数璀璨神奇的世界，踏上一段开拓的全新银河旅途！你将同乘员一起自由探索这片巨大星空下无限的奥秘。';
      case 'genshin-impact':
        return '在七种元素交汇的幻想世界——「提瓦特」大地中自由旅行，寻找掌控尘世元素的七神。在未知的终点，与久别分离的血亲再度重聚。';
      case 'zenless-zone-zero':
        return '米哈游自研的都市动作冒险游戏。故事发生在被神秘自然灾害「空洞」所侵袭的近未来，一座另类的奇迹都市——「新艾利都」悄然崛起，你将作为传奇「绳匠」探秘空洞。';
      case 'honkai-3rd':
        return '「为世界上所有的美好而战！」米哈游旗舰级3D动作探索游戏。与个性格异的 Valkyrie 女武神们并肩作战，齐心抵御神秘灾害『崩坏』，书写美丽传奇。';
      default:
        return '米哈游高品质精品自研游戏，感受卓越的故事剧情、爽快的动作战斗与沉浸式奇幻世界探索。';
    }
  };

  const trackRef = React.useRef<HTMLDivElement | null>(null);
  const scrollTargetRef = React.useRef<number | null>(null);
  const animationFrameRef = React.useRef<number | null>(null);
  const hoverTimerRef = React.useRef<any>(null);
  const enterTimerRef = React.useRef<any>(null);
  const leaveTimerRef = React.useRef<any>(null);

  const repeatedGames = [
    ...GAMES_DATA,
    ...GAMES_DATA,
    ...GAMES_DATA,
    ...GAMES_DATA,
    ...GAMES_DATA
  ];

  const handleCardMouseEnter = (gameId: string, index: number) => {
    if (enterTimerRef.current) clearTimeout(enterTimerRef.current);
    if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);

    // Instantly highlight the specific card's border and icon for quick visual feedback
    setHoveredCardIndex(index);

    // Elegant deliberate delay on background wallpaper & text transition to prevent flash-jump
    enterTimerRef.current = setTimeout(() => {
      setHoveredGameId(gameId);
      setDisplayedGameId(gameId);
    }, 240); // Deliberate delay to allow smooth gliding without flashing the description text
  };

  const handleCardMouseLeave = () => {
    if (enterTimerRef.current) clearTimeout(enterTimerRef.current);
    
    // Instantly clear the individual card highlight styling
    setHoveredCardIndex(null);

    // Safety buffer delay on reverting to the active game's details.
    // Confirms whether the user is moving between cards, keeping text and background steady.
    leaveTimerRef.current = setTimeout(() => {
      setHoveredGameId(null);
      setDisplayedGameId(activeGameId);
    }, 220); // Grace period ensures we stay stable if moving directly to another card
  };

  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (trackRef.current) {
      const container = trackRef.current;
      if (scrollTargetRef.current === null) {
        scrollTargetRef.current = container.scrollLeft;
      }
      
      // Accumulate scroll displacement
      scrollTargetRef.current += e.deltaY * 1.1; // Smoothly scale the scroll speed
      
      const animateScroll = () => {
        if (trackRef.current && scrollTargetRef.current !== null) {
          const current = trackRef.current.scrollLeft;
          const target = scrollTargetRef.current;
          const diff = target - current;
          
          if (Math.abs(diff) > 0.4) {
            // Decelerating spring/damping system for fluid momentum
            trackRef.current.scrollLeft += diff * 0.12;
            animationFrameRef.current = requestAnimationFrame(animateScroll);
          } else {
            trackRef.current.scrollLeft = target;
            scrollTargetRef.current = null;
            animationFrameRef.current = null;
          }
        }
      };
      
      if (animationFrameRef.current === null) {
        animationFrameRef.current = requestAnimationFrame(animateScroll);
      }
    }
  };

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const container = e.currentTarget;
    const itemWidth = 196; // 180 cards + 16 spacing (gap-4 is 16px)
    const singleSetWidth = itemWidth * GAMES_DATA.length;
    
    let wrapped = false;
    let delta = 0;
    
    if (container.scrollLeft >= singleSetWidth * 3) {
      delta = -singleSetWidth;
      wrapped = true;
    } else if (container.scrollLeft <= singleSetWidth) {
      delta = singleSetWidth;
      wrapped = true;
    }
    
    if (wrapped) {
      container.scrollLeft += delta;
      if (scrollTargetRef.current !== null) {
        scrollTargetRef.current += delta;
      }
    }
  };

  // Keep library scroll centered initially in the middle sets and handle hover resets
  useEffect(() => {
    if (isLibraryOpen) {
      setHoveredCardIndex(null);
      setHoveredGameId(null);
      setDisplayedGameId(activeGameId);
      if (enterTimerRef.current) clearTimeout(enterTimerRef.current);
      if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
      
      scrollTargetRef.current = null;
      if (trackRef.current) {
        const itemWidth = 196;
        const singleSetWidth = itemWidth * GAMES_DATA.length;
        // Position at start of the third set
        trackRef.current.scrollLeft = singleSetWidth * 2;
      }
    } else {
      setHoveredCardIndex(null);
      setHoveredGameId(null);
      if (enterTimerRef.current) clearTimeout(enterTimerRef.current);
      if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    }
  }, [isLibraryOpen, activeGameId]);

  // Clean up any lingering animations on component unmount
  useEffect(() => {
    return () => {
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
      if (enterTimerRef.current) clearTimeout(enterTimerRef.current);
      if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
      if (animationFrameRef.current) cancelAnimationFrame(animationFrameRef.current);
    };
  }, []);

  const activeWallpaper = isLibraryOpen && hoveredGameId
    ? (GAMES_DATA.find(g => g.id === hoveredGameId)?.bgImage || activeGame.bgImage)
    : activeGame.bgImage;

  // Audio Control States
  const [isAudioOn, setIsAudioOn] = useState<boolean>(false);
  const [bgmVolume, setBgmVolume] = useState<number>(0.5);

  // Download simulation speed limits
  const [speedLimit, setSpeedLimit] = useState<boolean>(false);

  // Sidebar visibility of game shortcut icons
  const [showGameIcons, setShowGameIcons] = useState<boolean>(true);
  const [imageErrors, setImageErrors] = useState<Record<string, boolean>>({});

  // Modal Dialog toggles
  const [selectedPost, setSelectedPost] = useState<GameNewsItem | null>(null);
  const [isPostOpen, setIsPostOpen] = useState<boolean>(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
  const [isExitAlertOpen, setIsExitAlertOpen] = useState<boolean>(false);
  const [isPlayerOpen, setIsPlayerOpen] = useState<boolean>(false);

  // Interactive Live Booting Loading State
  const [isLaunching, setIsLaunching] = useState<boolean>(false);
  const [launchProgress, setLaunchProgress] = useState<number>(0);
  const [launchStatusText, setLaunchStatusText] = useState<string>('');

  // Start BGM on user's first interaction or when game selection changes
  useEffect(() => {
    if (isAudioOn) {
      soundEngine.start(activeGameId);
    } else {
      soundEngine.stop();
    }
  }, [activeGameId, isAudioOn]);

  // Adjust volume when sliders are updated
  useEffect(() => {
    soundEngine.setVolume(bgmVolume);
  }, [bgmVolume]);

  const handleToggleAudio = () => {
    setIsAudioOn(!isAudioOn);
  };

  // Launch Game Dramatic Overlay Simulation
  const handleLaunchGame = () => {
    setIsLaunching(true);
    setLaunchProgress(0);
    
    const steps = [
      '正在连结星际轨道数据库...',
      '正在映射高保真着色材质...',
      '正在同步服务器网络状态与校验秘钥...',
      '星轨/传送锚点锁定中，祝您旅途愉快！',
    ];
    
    let currentStep = 0;
    setLaunchStatusText(steps[0]);

    const interval = setInterval(() => {
      setLaunchProgress(prev => {
        const next = prev + 5;
        
        // Progressively shift lore labels
        if (next === 25) { setLaunchStatusText(steps[1]); }
        if (next === 60) { setLaunchStatusText(steps[2]); }
        if (next === 85) { setLaunchStatusText(steps[3]); }

        if (next >= 100) {
          clearInterval(interval);
          setTimeout(() => {
            setIsLaunching(false);
            alert(`🎉 成功进入《${activeGame.name}》虚拟模拟大厅！\n游戏已完全运行。体验结束，返回启动器。`);
          }, 800);
          return 100;
        }
        return next;
      });
    }, 150);
  };

  const handleOpenPV = () => {
    setIsPlayerOpen(true);
  };

  const handleSelectNewsItem = (item: GameNewsItem) => {
    setSelectedPost(item);
    setIsPostOpen(true);
  };

  const handleSocialClick = (social: SocialLink) => {
    if (social.url.startsWith('#')) {
      alert(`🎉 模拟点击：《${activeGame.name}》- ${social.tooltip}`);
    } else {
      window.open(social.url, '_blank', 'noreferrer');
    }
  };

  const handleCloseLauncher = () => {
    setIsExitAlertOpen(true);
  };

  const handleConfirmExit = () => {
    setIsExitAlertOpen(false);
    alert('启动器已被模拟休眠/关闭。感谢您的操作！');
  };

  // Dynamic Theme Styling configurations
  const getAccentText = () => {
    switch (activeGame.accentColor) {
      case 'indigo': return 'text-indigo-400 group-hover:text-indigo-300';
      case 'yellow': return 'text-yellow-400 group-hover:text-yellow-300';
      default: return 'text-amber-400 group-hover:text-amber-300';
    }
  };

  const getAccentBg = () => {
    switch (activeGame.accentColor) {
      case 'indigo': return 'bg-indigo-500';
      case 'yellow': return 'bg-yellow-400';
      default: return 'bg-amber-400';
    }
  };

  return (
    <div id="pc-desktop" className="relative w-screen h-screen bg-[#07080a] flex items-center justify-center p-4 overflow-hidden select-none font-sans bg-radial from-slate-900/50 to-[#030406]">
      
      {/* Dynamic Animated background ambient circle for styling */}
      <div className="absolute top-1/4 left-1/4 w-[450px] h-[450px] rounded-full bg-gradient-to-tr from-amber-500/5 to-purple-800/10 blur-[80px] pointer-events-none animate-pulse" />
      <div className="absolute bottom-1/4 right-1/4 w-[500px] h-[500px] rounded-full bg-gradient-to-tr from-blue-500/5 to-emerald-600/5 blur-[90px] pointer-events-none" />

      {/* Launcher Window Frame Case wrapper with full wallpaper background */}
      <motion.div
        id="hoyoplay-launcher"
        layout
        className={`relative ${
          isFullscreen 
            ? 'w-full h-full rounded-none border-none' 
            : 'w-full max-w-[1150px] aspect-video rounded-3xl border border-white/10 shadow-3xl'
        } bg-[#111215] overflow-hidden flex flex-row transition-all duration-500 select-none`}
        style={{
          boxShadow: isFullscreen ? 'none' : '0 25px 60px -15px rgba(0, 0, 0, 0.94)'
        }}
      >

        {/* ========================================================= */}
        {/* FULL INTEGRATED BACKGROUND LAYER (Visible behind Sidebar)  */}
        {/* ========================================================= */}
        {/* WALLPAPER ARTWORK INTERACTIVE CANVAS WITH BREATHING LIFE */}
        <div className="absolute inset-0 z-0 select-none pointer-events-none overflow-hidden scale-105">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeWallpaper}
              initial={{ opacity: 0, scale: 1.01 }}
              animate={{ 
                opacity: 1, 
                scale: [1.01, 1.04, 1.01],
              }}
              exit={{ opacity: 0 }}
              transition={{ 
                opacity: { duration: 0.55 },
                scale: { 
                  duration: 26, 
                  ease: 'easeInOut', 
                  repeat: Infinity,
                  repeatType: 'reverse'
                }
              }}
              className="absolute inset-0"
            >
              <img
                src={activeWallpaper}
                alt={`${activeGame.name} Artwork`}
                referrerPolicy="no-referrer"
                className="w-full h-full object-cover select-none pointer-events-none"
              />
              {/* High precision atmospheric dark gradients for clear float readability */}
              <div className="absolute inset-0 bg-gradient-to-r from-black/85 via-black/30 to-black/10" />
              <div className="absolute inset-0 bg-gradient-to-t from-black/95 via-[#000000]/25 to-black/40" />
            </motion.div>
          </AnimatePresence>
        </div>

        {/* Dynamic Theme Star Dust drifting particles representing atmosphere status */}
        <div className="absolute inset-0 pointer-events-none overflow-hidden z-1">
          {Array.from({ length: 14 }).map((_, i) => {
            const size = Math.random() * 4.5 + 2.5; // 2.5px to 7px
            const duration = Math.random() * 18 + 14; // 14s to 32s
            const delay = Math.random() * -20; // Scattered start values
            const left = `${Math.random() * 100}%`;
            const opacity = Math.random() * 0.45 + 0.15;
            
            // Get gradient matching current active game theme
            const bulletColor = activeGame.id === 'star-rail' 
              ? 'rgba(254,211,48,0.75)' 
              : activeGame.id === 'genshin-impact' 
              ? 'rgba(230,92,0,0.75)' 
              : activeGame.id === 'zenless-zone-zero' 
              ? 'rgba(240,218,22,0.85)' 
              : 'rgba(204,43,125,0.75)';

            return (
              <motion.div
                key={i}
                className="absolute rounded-full pointer-events-none"
                style={{
                  width: size,
                  height: size,
                  left: left,
                  bottom: '-20px',
                  opacity: opacity,
                  background: `radial-gradient(circle, ${bulletColor} 0%, rgba(255,255,255,0) 100%)`,
                  boxShadow: `0 0 ${size * 1.5}px ${bulletColor}`,
                }}
                animate={{
                  y: ['0vh', '-110vh'],
                  x: ['0px', `${(Math.sin(i) * 35) + 15}px`, `${(Math.sin(i) * -35) - 15}px`, '0px'],
                }}
                transition={{
                  duration: duration,
                  repeat: Infinity,
                  ease: 'linear',
                  delay: delay,
                }}
              />
            );
          })}
        </div>

        {/* Top and side ambient glowing visual blobs from style mock */}
        <div className="absolute inset-0 z-0 pointer-events-none overflow-hidden">
          <div className="absolute top-10 right-20 w-[350px] h-[350px] bg-blue-500/10 blur-[130px] rounded-full" />
          <div className="absolute bottom-20 left-10 w-[250px] h-[250px] bg-purple-500/10 blur-[110px] rounded-full" />
        </div>

        {/* ========================================================= */}
        {/* LEFT NARROW GAME SHORTCUT SHORTCUTS SIDEBAR               */}
        {/* ========================================================= */}
        <div className="w-[58px] bg-[#111215]/12 backdrop-blur-lg border-r border-white/5 flex flex-col items-center py-6 shrink-0 z-40 relative select-none">
          {/* Top cute smile mascot logo matching HoYoPlay brand */}
          <motion.div
            whileHover={{ scale: 1.05 }}
            onClick={() => setIsSettingsOpen(true)}
            className="w-10 h-10 flex items-center justify-center cursor-pointer shrink-0"
            title="HoYoPlay 系统设置"
          >
            <svg viewBox="0 0 100 100" className="w-[34px] h-[34px] text-white fill-current">
              <circle cx="50" cy="52" r="22" fill="none" stroke="currentColor" strokeWidth="6" />
              <circle cx="42" cy="48" r="4" fill="currentColor" />
              <circle cx="58" cy="48" r="4" fill="currentColor" />
              <path d="M 44 58 Q 50 64 56 58" stroke="currentColor" strokeWidth="4" strokeLinecap="round" fill="none" />
              <rect x="20" y="44" width="7" height="16" rx="3.5" fill="currentColor" />
              <rect x="73" y="44" width="7" height="16" rx="3.5" fill="currentColor" />
              <path d="M 23 48 C 23 25, 77 25, 77 48" stroke="currentColor" strokeWidth="4.5" strokeLinecap="round" fill="none" />
              <path d="M 22 52 C 16 65, 30 78, 50 78 C 70 78, 84 65, 78 52" stroke="currentColor" strokeWidth="4.5" strokeLinecap="round" strokeLinejoin="round" fill="none" />
              <path d="M 72 26 Q 72 32 78 32 Q 72 32 72 38 Q 72 32 66 32 Q 72 32 72 26 Z" fill="currentColor" />
            </svg>
          </motion.div>

          {/* Spacer to push all games button to bottom */}
          <div className="flex-1" />

          {/* Separator line above All Games block */}
          <div className="w-[20px] h-[1.5px] bg-white/10 mb-3.5 shrink-0" />

          {/* All Games Library button in custom squircle style */}
          <div className="flex flex-col items-center w-full mb-2 select-none font-sans">
            {/* All Games button */}
            <div className="relative group flex items-center justify-center w-full">
              {/* Outer concentric halo ring (dark semi-transparent selection outline, matching reference perfectly) */}
              <div 
                id="sidebar-allgame-ring" 
                className={`absolute w-[44px] h-[44px] rounded-[13px] border-[2px] transition-all duration-200 pointer-events-none z-0 ${
                   isLibraryOpen 
                     ? 'border-black/65 opacity-100' 
                     : 'border-black/35 opacity-0 group-hover:opacity-100'
                }`} 
              />

              <motion.button
                id="btn-all-games-sidebar"
                onClick={() => setIsLibraryOpen(!isLibraryOpen)}
                whileTap={{ scale: 0.94 }}
                className={`w-9 h-9 rounded-[10px] flex items-center justify-center cursor-pointer transition-all duration-150 relative z-10 focus:outline-none bg-[#28292e] border-transparent`}
              >
                {/* Custom Sparkle Grid icon matching user screenshot exactly, centered and beautifully sized/styled */}
                <svg viewBox="0 0 24 24" className={`w-[21px] h-[21px] fill-current text-white/55 transition-all duration-200 ${
                  isLibraryOpen 
                    ? 'text-white opacity-100 font-bold' 
                    : 'group-hover:text-white group-hover:opacity-100'
                }`}>
                  {/* Top-Left squarcle */}
                  <rect x="3" y="3" width="7" height="7" rx="2" />
                  {/* Bottom-Left squarcle */}
                  <rect x="3" y="14" width="7" height="7" rx="2" />
                  {/* Bottom-Right squarcle */}
                  <rect x="14" y="14" width="7" height="7" rx="2" />
                  {/* Top-Right Sparkle/Star shape */}
                  <path d="M17.5 2.5 Q17.5 7 22 7 Q17.5 7 17.5 11.5 Q17.5 7 13 7 Q17.5 7 17.5 2.5 Z" />
                </svg>
              </motion.button>

              {/* White/grey light tooltip bubble block pointing right */}
              <div id="sidebar-allgame-tooltip" className="absolute left-[56px] top-1/2 -translate-y-1/2 scale-90 translate-x-[-4px] opacity-0 group-hover:scale-100 group-hover:translate-x-0 group-hover:opacity-100 pointer-events-none transition-all duration-200 z-50 flex items-center origin-left">
                {/* Outer speech bubble notch arrow */}
                <div className="w-2.5 h-2.5 rotate-45 bg-[#e3e5e9] relative -mr-1" style={{ borderRadius: '1.5px' }} />
                {/* Visual bubble label text */}
                <div className="bg-[#e3e5e9] text-[#121319] text-[11px] font-bold px-3 py-1.5 rounded-lg shadow-2xl whitespace-nowrap tracking-wide">
                  全部游戏
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* ========================================================= */}
        {/* MAIN DISPLAY CANVAS (Contains wallpapers, overlays)       */}
        {/* ========================================================= */}
        <div className="flex-1 h-full relative flex flex-col overflow-hidden">

          {/* HEADER: TITLE-BAR WINDOW CONTROLS */}
          <div className="relative z-40 h-16 w-full flex items-center justify-end px-8 select-none pointer-events-auto shrink-0">
            <WindowControls
              onOpenSettings={() => setIsSettingsOpen(true)}
              isFullscreen={isFullscreen}
              onToggleFullscreen={() => setIsFullscreen(!isFullscreen)}
              onCloseLauncher={handleCloseLauncher}
              isAudioOn={isAudioOn}
              onToggleAudio={handleToggleAudio}
            />
          </div>

          {/* ========================================================= */}
          {/* MAIN GRAPHICS SCENIC CANVAS CONTENT                      */}
          {/* ========================================================= */}
          <div className="absolute inset-0 z-30 pointer-events-none">
            
            {/* ========================================================= */}
            {/* DASHBOARD LAYER (Normal HomeScreen View)                  */}
            {/* ========================================================= */}
            <motion.div
              id="dashboard-intro-block"
              animate={{ 
                opacity: isLibraryOpen ? 0 : 1, 
                y: isLibraryOpen ? -50 : 0 
              }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'none' : 'auto' }}
              className="absolute top-12 left-10 z-40 flex flex-col space-y-3.5 select-none text-left max-w-[500px]"
            >
              {/* Big Game Character/Text logo */}
              <motion.div
                key={`logo-${activeGame.id}`}
                initial={{ opacity: 0, x: -10 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ duration: 0.4 }}
                className="flex items-center space-x-3 text-white mb-1.5"
              >
                {activeGame.id === 'star-rail' ? (
                  <div className="flex flex-col text-left text-white leading-none">
                    <span className="text-[28px] font-black tracking-tighter italic uppercase text-transparent bg-clip-text bg-gradient-to-r from-white to-gray-300">
                      崩坏：星穹铁道
                    </span>
                    <span className="text-[9px] uppercase tracking-[0.22em] text-[#fed330] font-bold font-mono mt-1">HONKAI: STAR RAIL</span>
                  </div>
                ) : activeGame.id === 'genshin-impact' ? (
                  <div className="flex flex-col text-left text-white leading-none">
                    <span className="text-[28px] font-extrabold tracking-tight font-sans drop-shadow-[0_2px_8px_rgba(0,0,0,0.5)]">
                      原神
                    </span>
                    <span className="text-[9px] uppercase tracking-[0.22em] text-[#fed330] font-bold font-mono mt-1">GENSHIN IMPACT</span>
                  </div>
                ) : activeGame.id === 'zenless-zone-zero' ? (
                  <div className="flex flex-col text-left text-white leading-none">
                    <span className="text-[28px] font-black italic tracking-tighter">
                      绝区零
                    </span>
                    <span className="text-[9px] uppercase tracking-[0.22em] text-[#fed330] font-bold font-mono mt-1">ZENLESS ZONE ZERO</span>
                  </div>
                ) : (
                  <div className="flex flex-col text-left text-white leading-none">
                    <span className="text-[28px] font-extrabold tracking-tight font-sans drop-shadow-[0_2px_8px_rgba(0,0,0,0.5)]">
                      崩坏3
                    </span>
                    <span className="text-[9px] uppercase tracking-[0.22em] text-[#fed330] font-bold font-mono mt-1">HONKAI IMPACT 3RD</span>
                  </div>
                )}
              </motion.div>

              {/* Banner text block: fits HoyoPlay reference layout perfectly */}
              <motion.div
                key={`banner-${activeGame.id}`}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5, delay: 0.1 }}
                className="space-y-2.5"
              >
                {/* Event Category Badge with checkered pattern */}
                <div className="flex items-center">
                  <div className={`px-2.5 py-[3px] text-[11px] font-bold tracking-wide uppercase text-white flex items-center shadow-md select-none rounded-[3px] leading-none ${
                    activeGame.id === 'star-rail' 
                      ? 'bg-[#c23c3c]' 
                      : activeGame.id === 'genshin-impact' 
                      ? 'bg-[#e65c00]' 
                      : activeGame.id === 'zenless-zone-zero' 
                      ? 'bg-[#f0da16] text-[#121319]' 
                      : 'bg-[#cc2b7d]'
                  }`}>
                    {activeGame.bannerCategory ? activeGame.bannerCategory.replace(/[ ❖| ❖| ✦]/g, '') : '推荐活动'}
                  </div>
                  
                  {/* Grid design decoration representing Hoyo style precisely */}
                  <div className="flex flex-col space-y-[2px] ml-2">
                    <div className="flex space-x-[2px]">
                      <div className={`w-[4.5px] h-[4.5px] rounded-[1px] ${
                        activeGame.id === 'star-rail' ? 'bg-[#c23c3c]' : activeGame.id === 'genshin-impact' ? 'bg-[#e65c00]' : activeGame.id === 'zenless-zone-zero' ? 'bg-[#f0da16]' : 'bg-[#cc2b7d]'
                      }`} />
                      <div className="w-[4.5px] h-[4.5px] bg-transparent" />
                      <div className={`w-[4.5px] h-[4.5px] rounded-[1px] ${
                        activeGame.id === 'star-rail' ? 'bg-[#c23c3c]' : activeGame.id === 'genshin-impact' ? 'bg-[#e65c00]' : activeGame.id === 'zenless-zone-zero' ? 'bg-[#f0da16]' : 'bg-[#cc2b7d]'
                      }`} />
                    </div>
                    <div className="flex space-x-[2px]">
                      <div className="w-[4.5px] h-[4.5px] bg-transparent" />
                      <div className={`w-[4.5px] h-[4.5px] rounded-[1px] ${
                        activeGame.id === 'star-rail' ? 'bg-[#c23c3c]' : activeGame.id === 'genshin-impact' ? 'bg-[#e65c00]' : activeGame.id === 'zenless-zone-zero' ? 'bg-[#f0da16]' : 'bg-[#cc2b7d]'
                      }`} />
                      <div className="w-[4.5px] h-[4.5px] bg-transparent" />
                    </div>
                  </div>
                </div>
                
                {/* Large display headline with vertical white-to-light-gold text gradient */}
                <h1 
                  className="text-4xl sm:text-[48px] font-black tracking-wide leading-tight select-none"
                  style={{
                    background: 'linear-gradient(to bottom, #ffffff 15%, #ffffff 50%, #f1e2c3 100%)',
                    WebkitBackgroundClip: 'text',
                    WebkitTextFillColor: 'transparent',
                    filter: 'drop-shadow(0 4px 10px rgba(0,0,0,0.92)) drop-shadow(0 1px 2px rgba(0,0,0,1))',
                  }}
                >
                  {activeGame.bannerTitle}
                </h1>
                
                {/* Adaptive Timing details styled carefully */}
                <p 
                  className="text-[13.5px] sm:text-[14.5px] font-bold text-[#ebe4d0] tracking-wide select-none"
                  style={{
                    textShadow: '0 2px 4px rgba(0,0,0,0.95), 0 1px 1px rgba(0,0,0,1)'
                  }}
                >
                  {activeGame.bannerSubtitle}
                </p>
              </motion.div>

              {/* Play Action Button: Beautiful White Pill */}
              <div className="pt-2">
                <motion.button
                  whileHover={{ scale: 1.04, y: -1 }}
                  whileTap={{ scale: 0.97 }}
                  onClick={handleOpenPV}
                  className="bg-white hover:bg-neutral-100 text-[#121319] hover:text-black font-extrabold text-[13.5px] px-[28px] py-[10.5px] rounded-full shadow-[0_8px_20px_rgba(255,255,255,0.18)] transition-all cursor-pointer border-none flex items-center justify-center space-x-1.5 focus:outline-none"
                >
                  <span>{activeGame.bannerButtonText || '查看活动'}</span>
                </motion.button>
              </div>
            </motion.div>

            {/* Dashboard Bottom-Left NewsCard block */}
            <motion.div
              id="dashboard-news-block"
              animate={{ 
                opacity: isLibraryOpen ? 0 : 1, 
                y: isLibraryOpen ? 280 : 0 
              }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'none' : 'auto' }}
              className="absolute bottom-[20px] left-8 z-30 select-none flex flex-col space-y-4"
            >
              <NewsCard
                pvCardImage={activeGame.pvCardImage}
                pvTitle={activeGame.pvTitle}
                news={activeGame.news}
                accentColor={activeGame.accentColor}
                onOpenPV={handleOpenPV}
                onSelectNews={handleSelectNewsItem}
              />
            </motion.div>

            {/* Float Right Social sidebar - hidden smoothly when library (All Games interface) is open */}
            <motion.div
              id="float-social-sidebar"
              animate={{ 
                opacity: isLibraryOpen ? 0 : 1,
                scale: isLibraryOpen ? 0.85 : 1,
                x: isLibraryOpen ? 30 : 0
              }}
              transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'none' : 'auto' }}
              className="absolute right-[14px] top-[41%] -translate-y-1/2 z-40 select-none"
            >
              <SocialSidebar
                socials={activeGame.socials}
                onSocialClick={handleSocialClick}
              />
            </motion.div>

            {/* Dashboard Bottom-Right Download Button section */}
            <motion.div
              id="dashboard-download-block"
              animate={{ 
                opacity: isLibraryOpen ? 0 : 1, 
                y: isLibraryOpen ? 280 : 0 
              }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'none' : 'auto' }}
              className="absolute bottom-[28px] right-8 z-35 select-none flex flex-col items-center font-sans"
            >
              <DownloadButton
                gameId={activeGame.id}
                gameName={activeGame.name}
                sizeGB={activeGame.sizeGB}
                speedMB={activeGame.installSpeedMB}
                accentColor={activeGame.accentColor}
                onLaunchGame={handleLaunchGame}
              />
            </motion.div>

            {/* ========================================================= */}
            {/* LIBRARY LAYER: TOP BRANDING LOGO (上面的艺术字)           */}
            {/* ========================================================= */}
            <motion.div
              id="library-intro-block"
              animate={{ 
                opacity: isLibraryOpen ? 1 : 0, 
                y: isLibraryOpen ? 0 : -50 
              }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'auto' : 'none' }}
              className="absolute top-20 left-10 z-40 flex flex-col select-none text-left"
            >
              <AnimatePresence mode="wait">
                <motion.div
                  key={`lib-logo-${displayedGameId}`}
                  initial={{ opacity: 0, y: 15 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -15 }}
                  transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
                >
                  {displayedGameId === 'star-rail' ? (
                    <div className="flex flex-col text-left text-white leading-none">
                      <span className="text-4xl sm:text-[45px] font-black tracking-tighter italic uppercase text-transparent bg-clip-text bg-gradient-to-r from-white via-white to-gray-300 drop-shadow-[0_4px_12px_rgba(255,255,255,0.08)] select-none">
                        崩坏：星穹铁道
                      </span>
                      <span className="text-[12px] uppercase tracking-[0.25em] text-[#fed330] font-black font-mono mt-1.5 select-none">
                        HONKAI: STAR RAIL
                      </span>
                    </div>
                  ) : displayedGameId === 'genshin-impact' ? (
                    <div className="flex flex-col text-left text-white leading-none">
                      <span className="text-4xl sm:text-[45px] font-extrabold tracking-tight drop-shadow-[0_4px_15px_rgba(255,255,255,0.12)] select-none font-sans">
                        原神
                      </span>
                      <span className="text-[11px] uppercase tracking-[0.32em] text-[#fed330] font-black font-sans mt-2 select-none">
                        GENSHIN IMPACT
                      </span>
                    </div>
                  ) : displayedGameId === 'zenless-zone-zero' ? (
                    <div className="flex flex-col text-left text-white leading-none">
                      <span className="text-4xl sm:text-[46px] font-black italic tracking-tighter text-transparent bg-clip-text bg-gradient-to-r from-yellow-300 via-white to-gray-200 drop-shadow-[0_4px_12px_rgba(254,211,48,0.12)] select-none">
                        绝区零
                      </span>
                      <span className="text-[11px] uppercase tracking-[0.24em] text-[#fed330] font-black font-mono mt-1.5 select-none">
                        ZENLESS ZONE ZERO
                      </span>
                    </div>
                  ) : (
                    <div className="flex flex-col text-left text-white leading-none">
                      <span className="text-4xl sm:text-[46px] font-black tracking-tight italic drop-shadow-[0_4px_16px_rgba(255,255,255,0.18)] flex items-end select-none leading-none">
                        崩坏<span className="text-[#fed330] text-3xl sm:text-[38px] font-extrabold not-italic ml-1 mb-0.5 leading-none">3</span>
                      </span>
                      <span className="text-[10px] uppercase tracking-[0.3em] text-[#fed330] font-black font-mono mt-2.5 select-none">
                        HONKAI IMPACT 3RD
                      </span>
                    </div>
                  )}
                </motion.div>
              </AnimatePresence>
            </motion.div>

            {/* ========================================================= */}
            {/* LIBRARY LAYER: BOTTOM TEXT BLOCK (下边的文本放在下面)       */}
            {/* ========================================================= */}
            <motion.div
              id="library-description-block"
              animate={{ 
                opacity: isLibraryOpen ? 1 : 0, 
                y: isLibraryOpen ? 0 : 40 
              }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'auto' : 'none' }}
              className="absolute bottom-[170px] left-10 z-40 flex flex-col select-none text-left max-w-[650px]"
            >
              <AnimatePresence mode="wait">
                <motion.div
                  key={`lib-desc-${displayedGameId}`}
                  initial={{ opacity: 0, y: 15 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -15 }}
                  transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
                >
                  <p className="text-[15px] font-medium text-gray-200 leading-relaxed drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)] font-sans antialiased">
                    {getGameDescription(displayedGameId)}
                  </p>
                </motion.div>
              </AnimatePresence>
            </motion.div>

            <motion.div
              id="library-carousel-block"
              animate={{ 
                opacity: isLibraryOpen ? 1 : 0, 
                y: isLibraryOpen ? 0 : 280 
              }}
              transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
              style={{ pointerEvents: isLibraryOpen ? 'auto' : 'none' }}
              className="absolute bottom-6 left-8 right-8 z-40 select-none"
            >
              <div className="relative w-full overflow-hidden">
                {/* Fade transparent linear mask shields */}
                <div className="absolute left-0 top-0 bottom-0 w-16 bg-gradient-to-r from-[#111215] to-transparent pointer-events-none z-10" />
                <div className="absolute right-0 top-0 bottom-0 w-16 bg-gradient-to-l from-[#111215] to-transparent pointer-events-none z-10" />

                {/* Horizontal scroll track with onWheel and wrapping handlers */}
                <div 
                  ref={trackRef}
                  onWheel={handleWheel}
                  onScroll={handleScroll}
                  className="flex items-center space-x-4 overflow-x-auto py-2 px-12 scrollbar-none"
                  style={{ scrollBehavior: 'auto' }}
                >
                  {repeatedGames.map((g, index) => {
                    const isSelected = g.id === activeGameId;
                    const isCardHovered = index === hoveredCardIndex;
                    
                    return (
                      <div 
                        key={`card-col-${g.id}-${index}`} 
                        className="flex flex-col items-center space-y-2.5 shrink-0"
                      >
                        <motion.div
                          id={`card-selector-${g.id}-${index}`}
                          onMouseEnter={() => handleCardMouseEnter(g.id, index)}
                          onMouseLeave={handleCardMouseLeave}
                          onClick={() => {
                            setActiveGameId(g.id);
                            setIsLibraryOpen(false);
                            if (isAudioOn) {
                              soundEngine.start(g.id);
                            }
                          }}
                          whileHover={{ y: -6, scale: 1.02 }}
                          whileTap={{ scale: 0.98 }}
                          className={`relative w-[180px] h-[95px] rounded-xl overflow-hidden cursor-pointer shrink-0 transition-[border-color,opacity,box-shadow] duration-200 border ${
                            isSelected
                              ? 'border-[#ffeb3b] shadow-[0_0_20px_rgba(255,235,59,0.35)] ring-1 ring-[#ffeb3b]/40 opacity-100'
                              : isCardHovered
                              ? 'border-[#ffeb3b] shadow-[0_0_15px_rgba(255,235,59,0.25)] opacity-100'
                              : 'border-white/10 opacity-70'
                          }`}
                        >
                          {/* Thumbnail Cover image */}
                          <img
                            src={g.pvCardImage}
                            alt={g.name}
                            referrerPolicy="no-referrer"
                            className="w-full h-full object-cover"
                          />
                          
                          {/* Semi translucent bottom card gradient shield */}
                          <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/20 to-transparent" />
                          
                          {/* Badge representing Active status */}
                          {isSelected && (
                            <div className="absolute top-1.5 right-1.5 px-2 py-0.5 rounded bg-yellow-400 text-black text-[9px] font-black uppercase tracking-wider block z-10">
                              使用中
                            </div>
                          )}

                          {/* Hover Details overlay matching HoyoPlay screenshot exactly */}
                          <AnimatePresence>
                            {isCardHovered && (
                              <motion.div 
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                exit={{ opacity: 0 }}
                                transition={{ duration: 0.16, ease: "easeOut" }}
                                className="absolute inset-0 bg-black/75 flex items-center justify-center pointer-events-none z-20"
                              >
                                <motion.div
                                  initial={{ scale: 0.94, opacity: 0 }}
                                  animate={{ scale: 1, opacity: 1 }}
                                  exit={{ scale: 0.94, opacity: 0 }}
                                  transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
                                  className="flex items-center justify-center space-x-2"
                                >
                                  {/* Fixed-sized container wrapper so layout never shifts */}
                                  <div className="w-7 h-7 rounded-[8px] border border-white/10 bg-white/10 flex items-center justify-center overflow-hidden shrink-0 relative">
                                    <img 
                                      src={GAME_ICON_MAP[g.id]} 
                                      alt={g.name} 
                                      referrerPolicy="no-referrer"
                                      className="w-full h-full object-cover absolute inset-0 z-10"
                                      onError={(e) => {
                                        e.currentTarget.style.opacity = '0';
                                      }}
                                    />
                                    {/* Built-in high visual-fidelity vector fallback structure */}
                                    <div className="absolute inset-0 flex items-center justify-center text-white/40 z-0">
                                      <Gamepad className="w-3.5 h-3.5" />
                                    </div>
                                  </div>
                                  <span className="text-[#ffeb3b] text-[13px] font-bold tracking-wide">
                                    查看详情
                                  </span>
                                </motion.div>
                              </motion.div>
                            )}
                          </AnimatePresence>
                        </motion.div>

                        {/* Title text labels BELOW the card, centered precisely */}
                        <div className="text-center w-full">
                          <span className={`text-[12px] font-bold block tracking-wide select-none transition-colors duration-200 ${
                            isSelected 
                              ? 'text-[#ffeb3b] font-extrabold' 
                              : isCardHovered 
                              ? 'text-white' 
                              : 'text-[#9fa2b4]'
                          }`}>
                            {g.name}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            </motion.div>
          </div>

        </div>

        {/* ========================================================= */}
        {/* ALL DIALOGS & OVERLAY INTERACTIVE ELEMENTS                */}
        {/* ========================================================= */}

        {/* Character PV Video Player */}
        <VideoPlayer
          isOpen={isPlayerOpen}
          onClose={() => setIsPlayerOpen(false)}
          title={activeGame.pvTitle}
          posterImage={activeGame.pvCardImage}
        />

        {/* News detail forum viewer */}
        <PostDetailModal
          isOpen={isPostOpen}
          onClose={() => {
            setIsPostOpen(false);
            setSelectedPost(null);
          }}
          post={selectedPost}
          gameName={activeGame.name}
        />

        {/* Launcher Configuration parameters Modal */}
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

        {/* Classic Confirm Quit / minimize dialog box */}
        <ConfirmExitModal
          isOpen={isExitAlertOpen}
          onCancel={() => setIsExitAlertOpen(false)}
          onConfirmExit={handleConfirmExit}
        />

        {/* Dynamic Game cinematic screen Loading Overlays */}
        <AnimatePresence>
          {isLaunching && (
            <motion.div
              id="game-launch-curtain"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="absolute inset-0 bg-[#000] z-50 flex flex-col items-center justify-center p-6"
            >
              {/* Inner floating game loading logo */}
              <motion.div
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                className="flex flex-col items-center space-y-7 max-w-md text-center"
              >
                {/* Loader Spinner */}
                <Loader2 className="w-10 h-10 text-yellow-400 animate-spin opacity-80" />

                <div className="space-y-2">
                  {/* Styled loading subtitle logo brand */}
                  <h2 className="text-3xl font-extrabold tracking-widest text-[#fff]">
                    {activeGame.name}
                  </h2>
                  <p className="text-[11px] uppercase tracking-[0.25em] text-gray-500 font-mono">
                    {activeGame.enName}
                  </p>
                </div>

                {/* Progress bar container */}
                <div className="w-[280px] h-1.5 bg-neutral-900 rounded-full overflow-hidden border border-white/5 relative">
                  <motion.div 
                    className="h-full bg-gradient-to-r from-yellow-400 via-amber-300 to-yellow-500 rounded-full"
                    style={{ width: `${launchProgress}%` }}
                    transition={{ ease: 'easeOut' }}
                  />
                </div>

                {/* Simulated dynamic lore tips or server messages block */}
                <span className="text-[13px] text-gray-400 block h-6 font-medium animate-pulse">
                  {launchStatusText}
                </span>

                {/* Subtitle instructions */}
                <span className="text-[11px] text-gray-600 block mt-4 font-mono">
                  资源下载进度：{launchProgress}% · 加载中...
                </span>
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>

      </motion.div>
    </div>
  );
}
