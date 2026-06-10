import React, { useState, useEffect, useRef } from 'react';
import { Download, Play, Pause, RefreshCw, CheckCircle2 } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';

interface DownloadButtonProps {
  gameId: string;
  gameName: string;
  sizeGB: number;
  speedMB: number;
  accentColor: string;
  onLaunchGame: () => void;
}

type InstallStatus = 'uninstalled' | 'downloading' | 'paused' | 'verifying' | 'installed';

export default function DownloadButton({
  gameId,
  gameName,
  sizeGB,
  speedMB,
  accentColor,
  onLaunchGame,
}: DownloadButtonProps) {
  const [status, setStatus] = useState<InstallStatus>('uninstalled');
  const [progress, setProgress] = useState(0); // 0 to 100
  const [currentSpeed, setCurrentSpeed] = useState(speedMB);
  const [timeLeftStr, setTimeLeftStr] = useState('');
  
  const progressIntervalRef = useRef<NodeJS.Timeout | null>(null);

  // Clear interval on unmount or game change
  useEffect(() => {
    return () => {
      if (progressIntervalRef.current) clearInterval(progressIntervalRef.current);
    };
  }, []);

  // Reset download state if game changes
  useEffect(() => {
    setStatus('uninstalled');
    setProgress(0);
    if (progressIntervalRef.current) {
      clearInterval(progressIntervalRef.current);
      progressIntervalRef.current = null;
    }
  }, [gameId]);

  // Start Downloading action
  const handleStartDownload = () => {
    if (status === 'uninstalled' || status === 'paused') {
      setStatus('downloading');
      
      const updateInterval = 400; // ms
      progressIntervalRef.current = setInterval(() => {
        setProgress((prev) => {
          // Speed fluctuation
          const randomFluctuation = (Math.random() - 0.5) * 8; // -4MB to +4MB
          const updatedSpeed = Math.max(10, speedMB + randomFluctuation);
          setCurrentSpeed(updatedSpeed);

          // Calculate new progress step based on simulated file size & speed
          // sizeGB * 1024 (MB) is total. updatedSpeed is MB/sec.
          // in 400ms, downloaded is updatedSpeed * 0.4 MB
          const totalMB = sizeGB * 1024;
          const downloadedStepMB = updatedSpeed * 0.4;
          const stepPercent = (downloadedStepMB / totalMB) * 100;
          
          let nextProgress = prev + stepPercent;

          if (nextProgress >= 100) {
            nextProgress = 100;
            clearInterval(progressIntervalRef.current!);
            progressIntervalRef.current = null;
            
            // Proceed to verifying
            setTimeout(() => {
              setStatus('verifying');
              let verifyProgress = 0;
              const verifyTimer = setInterval(() => {
                verifyProgress += 10;
                if (verifyProgress >= 100) {
                  clearInterval(verifyTimer);
                  setStatus('installed');
                }
              }, 120);
            }, 600);
          }

          // Calculate time remaining representation
          const remainingPercent = 100 - nextProgress;
          const remainingMB = (remainingPercent / 100) * totalMB;
          const remainingSeconds = remainingMB / updatedSpeed;
          
          if (remainingSeconds < 60) {
            setTimeLeftStr(`约 ${Math.ceil(remainingSeconds)} 秒`);
          } else {
            const mins = Math.floor(remainingSeconds / 60);
            const secs = Math.ceil(remainingSeconds % 60);
            setTimeLeftStr(`约 ${mins} 分 ${secs} 秒`);
          }

          return nextProgress;
        });
      }, updateInterval);
    }
  };

  const handlePauseDownload = () => {
    if (status === 'downloading') {
      setStatus('paused');
      if (progressIntervalRef.current) {
        clearInterval(progressIntervalRef.current);
        progressIntervalRef.current = null;
      }
    }
  };

  const getAccentBtnBg = () => {
    // HoYoPlay uses a beautiful, unified premium gold yellow button style for the launcher primary actions
    return 'bg-[#fed330] hover:bg-[#ffe055] text-[#121319] border-none shadow-[0_4px_20px_rgba(254,211,48,0.3)] transition-all duration-200 select-none';
  };

  const getAccentBarColor = () => {
    switch (accentColor) {
      case 'indigo':
        return 'bg-indigo-500';
      case 'yellow':
        return 'bg-yellow-400';
      default:
        return 'bg-yellow-400';
    }
  };

  return (
    <div className="flex flex-col items-center">
      <AnimatePresence mode="wait">
        {/* State 1: Uninstalled (Get Game / 获取游戏) */}
        {status === 'uninstalled' && (
          <motion.div
            key="uninstalled"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="flex flex-col items-center space-y-2"
          >
            <motion.button
              id="btn-install-game"
              onClick={handleStartDownload}
              whileHover={{ scale: 1.03, y: -1 }}
              whileTap={{ scale: 0.97 }}
              className="group/btn w-[172px] h-[52px] rounded-full flex items-center justify-start pl-[11px] pr-[16px] transition-all duration-200 cursor-pointer select-none no-underline border-none bg-[#fed330] hover:bg-[#252932] text-[#121319] hover:text-[#fed330] shadow-[0_4px_16px_rgba(254,211,48,0.25)] hover:shadow-[0_4px_16px_rgba(37,41,50,0.3)] focus:outline-none"
            >
              {/* Left circular panel enclosing the looping/bouncing downward arrow */}
              <div className="w-8 h-8 rounded-full bg-[#121319] group-hover/btn:bg-[#fed330] flex items-center justify-center mr-2.5 shrink-0 relative transition-colors duration-200">
                <motion.svg
                  viewBox="0 0 24 24"
                  className="w-4 h-4 text-[#fed330] group-hover/btn:text-[#121319] transition-colors duration-200"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="3.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  animate={{
                    y: [-2.5, 2.5, -2.5],
                  }}
                  transition={{
                    duration: 1.4,
                    repeat: Infinity,
                    ease: "easeInOut"
                  }}
                >
                  <line x1="12" y1="5" x2="12" y2="19" />
                  <polyline points="19 12 12 19 5 12" />
                </motion.svg>
              </div>
              <span className="font-extrabold text-[16.5px] tracking-wide leading-none select-none no-underline flex-1 text-center pr-1">获取游戏</span>
            </motion.button>

            {/* Path Locator Link formatted nicely inside flex layout */}
            <div className="text-[13px] text-white/85 flex items-center space-x-1 font-sans">
              <span>已安装？</span>
              <button
                id="locate-game-link"
                onClick={() => setStatus('installed')}
                className="text-[#fed330] hover:text-[#ffe055] font-semibold cursor-pointer transition-colors focus:outline-none bg-transparent border-none p-0 inline-block"
              >
                定位游戏
              </button>
            </div>
          </motion.div>
        )}

        {/* State 2: Downloading & Paused System */}
        {(status === 'downloading' || status === 'paused') && (
          <motion.div
            key="downloading-state"
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            className="w-[240px] bg-black/60 border border-white/10 backdrop-blur-md rounded-2xl p-4 flex flex-col space-y-2.5 shadow-2xl z-30"
          >
            {/* Speed, Name & Control row */}
            <div className="flex items-center justify-between">
              <div>
                <span className="text-white text-xs font-bold tracking-wide">
                  {status === 'downloading' ? '正在获取资源...' : '已暂停'}
                </span>
                <p className="text-[10px] text-gray-400 font-mono leading-none mt-0.5">
                  {status === 'downloading' ? `${currentSpeed.toFixed(1)} MB/s` : '0.0 MB/s'}
                </p>
              </div>

              {/* Pause/Play Controls */}
              <div className="flex space-x-1.5">
                {status === 'downloading' ? (
                  <button
                    id="download-pause-btn"
                    onClick={handlePauseDownload}
                    className="w-6 h-6 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center text-white transition-colors cursor-pointer"
                    title="暂停下载"
                  >
                    <Pause className="w-3 h-3" />
                  </button>
                ) : (
                  <button
                    id="download-resume-btn"
                    onClick={handleStartDownload}
                    className="w-6 h-6 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center text-white transition-colors cursor-pointer"
                    title="继续下载"
                  >
                    <Download className="w-3 h-3" />
                  </button>
                )}
              </div>
            </div>

            {/* Progress Track */}
            <div className="relative w-full h-2 bg-white/10 rounded-full overflow-hidden">
              <div
                className={`h-full absolute left-0 top-0 rounded-full transition-all duration-300 ${getAccentBarColor()}`}
                style={{ width: `${progress}%` }}
              />
            </div>

            {/* Metrics Footer */}
            <div className="flex items-center justify-between text-[9px] font-mono text-gray-400">
              <span>{progress.toFixed(1)}%</span>
              <span>{timeLeftStr}</span>
            </div>
          </motion.div>
        )}

        {/* State 3: Verifying */}
        {status === 'verifying' && (
          <motion.div
            key="verifying"
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            className="w-[240px] bg-black/60 border border-white/10 backdrop-blur-md rounded-2xl p-4 flex flex-col items-center justify-center space-y-3.5 shadow-2xl z-30"
          >
            <RefreshCw className="w-6 h-6 text-[#fed330] animate-spin" />
            <div className="text-center">
              <span className="text-white text-xs font-bold tracking-wide block">
                正在校验游戏文件...
              </span>
            </div>
          </motion.div>
        )}

        {/* State 4: Installed -> READY (开始游戏 / Start Game) */}
        {status === 'installed' && (
          <motion.div
            key="installed"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="flex flex-col items-center space-y-2"
          >
            <motion.button
              id="btn-launch-game"
              onClick={onLaunchGame}
              whileHover={{ scale: 1.03, y: -1 }}
              whileTap={{ scale: 0.97 }}
              className="group/btn w-[172px] h-[52px] rounded-full flex items-center justify-start pl-[11px] pr-[16px] transition-all duration-200 cursor-pointer select-none no-underline border-none bg-[#fed330] hover:bg-[#252932] text-[#121319] hover:text-[#fed330] shadow-[0_4px_16px_rgba(254,211,48,0.25)] hover:shadow-[0_4px_16px_rgba(37,41,50,0.3)] focus:outline-none"
            >
              <div className="w-8 h-8 rounded-full bg-[#121319] group-hover/btn:bg-[#fed330] flex items-center justify-center mr-2.5 shrink-0 relative transition-colors duration-200 ml-0.5 animate-none">
                <svg
                  viewBox="0 0 24 24"
                  className="w-4 h-4 fill-current text-[#fed330] group-hover/btn:text-[#121319] transition-colors duration-200 ml-0.5"
                >
                  <path d="M8 5v14l11-7z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
                </svg>
              </div>
              <span className="font-extrabold text-[16.5px] tracking-wide leading-none select-none no-underline flex-1 text-center pr-1">开始游戏</span>
            </motion.button>

            {/* Sub-text ready message formatted nicely inside flex layout */}
            <div className="text-[11px] text-white/45 flex items-center space-x-1.5 font-sans">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
              <span>当前已是最新版本 &bull; 随时可以启动</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
