import React from 'react';
import { motion } from 'motion/react';

interface WindowControlsProps {
  onOpenSettings: () => void;
  isFullscreen: boolean;
  onToggleFullscreen: () => void;
  onCloseLauncher: () => void;
  isAudioOn: boolean;
  onToggleAudio: () => void;
}

export default function WindowControls({
  onOpenSettings,
  isFullscreen,
  onToggleFullscreen,
  onCloseLauncher,
  isAudioOn,
  onToggleAudio,
}: WindowControlsProps) {
  return (
    <div className="absolute top-5 right-7 flex items-center space-x-1.5 z-50 select-none">
      {/* Sound Controller Button representing atmosphere status */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-bgm-sound-controller-top"
          onClick={onToggleAudio}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-[#fed330] hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          {isAudioOn ? (
            /* Custom active sound waves symbol */
            <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M11 5L6 9H2v6h4l5 4V5z" fill="currentColor" />
              <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
              <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
            </svg>
          ) : (
            /* Muted custom sound wave with slash */
            <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none opacity-60" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M11 5L6 9H2v6h4l5 4V5z" />
              <line x1="23" y1="9" x2="17" y2="15" />
              <line x1="17" y1="9" x2="23" y2="15" />
            </svg>
          )}
        </motion.button>

        {/* Custom White Tooltip (Hanging underneath, pointing up) */}
        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            {/* Triangular arrow anchor */}
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10">{isAudioOn ? "静音背景乐" : "播放背景乐"}</span>
          </div>
        </div>
      </div>

      {/* Dynamic Hex Nut Settings Button matching Image 4 */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-settings"
          onClick={onOpenSettings}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-white hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 2L20.66 7V17L12 22L3.34 17V7L12 2Z" />
            <circle cx="12" cy="12" r="3.2"/>
          </svg>
        </motion.button>

        {/* Custom White Tooltip matching Image 4 exactly */}
        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            {/* Triangular arrow anchor */}
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10 tracking-wide">设置</span>
          </div>
        </div>
      </div>

      {/* Minimize Button matching Image 3 */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-minimize"
          onClick={() => {
            alert('提示：启动器已最小化至系统托盘 (模拟)');
          }}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-white hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.8" strokeLinecap="round">
            <line x1="4" y1="12" x2="20" y2="12" />
          </svg>
        </motion.button>

        {/* Custom White Tooltip */}
        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            {/* Triangular arrow anchor */}
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10">最小化</span>
          </div>
        </div>
      </div>

      {/* Close Button with clean look from Image 3 */}
      <div className="relative group flex flex-col items-center">
        <motion.button
          id="btn-close"
          onClick={onCloseLauncher}
          whileTap={{ scale: 0.92 }}
          className="text-[#cccccc] hover:text-red-400 hover:bg-white/10 transition-all cursor-pointer focus:outline-none flex items-center justify-center w-8 h-8 rounded-md"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px] stroke-current fill-none" strokeWidth="2.5" strokeLinecap="round">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </motion.button>

        {/* Custom White Tooltip */}
        <div className="absolute top-[130%] left-1/2 -translate-x-1/2 opacity-0 scale-90 pointer-events-none group-hover:opacity-100 group-hover:scale-100 transition-all duration-150 z-50">
          <div className="relative bg-white text-black text-[12px] font-bold py-[3.5px] px-[12px] shadow-lg rounded-[5px] whitespace-nowrap font-sans">
            {/* Triangular arrow anchor */}
            <div className="absolute -top-1 left-1/2 -translate-x-1/2 w-2 h-2 rotate-45 bg-white" />
            <span className="relative z-10">关闭</span>
          </div>
        </div>
      </div>
    </div>
  );
}

