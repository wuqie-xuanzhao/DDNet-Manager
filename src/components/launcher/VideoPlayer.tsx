import { AnimatePresence, motion } from 'framer-motion';
import { useEffect, useRef, useState, type ChangeEvent } from 'react';
import { X, Play, Pause, Volume2, Maximize, RotateCcw } from 'lucide-react';

interface VideoPlayerProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  posterImage: string;
}

export default function VideoPlayer({ isOpen, onClose, title, posterImage }: VideoPlayerProps) {
  const [isPlaying, setIsPlaying] = useState(true);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration] = useState(84); // 1 minute 24 seconds simulated video length
  const [volume, setVolume] = useState(0.8);
  const progressIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (isOpen && isPlaying) {
      progressIntervalRef.current = setInterval(() => {
        setCurrentTime((prev) => {
          if (prev >= duration) {
            if (progressIntervalRef.current) {
              clearInterval(progressIntervalRef.current);
            }
            setIsPlaying(false);
            return duration;
          }
          return prev + 1;
        });
      }, 1000);
    } else {
      if (progressIntervalRef.current) {
        clearInterval(progressIntervalRef.current);
      }
    }

    return () => {
      if (progressIntervalRef.current) clearInterval(progressIntervalRef.current);
    };
  }, [isOpen, isPlaying, duration]);

  // Reset clock when closed or opened
  useEffect(() => {
    if (isOpen) {
      setCurrentTime(0);
      setIsPlaying(true);
    }
  }, [isOpen]);

  const formatTime = (seconds: number) => {
    const min = Math.floor(seconds / 60);
    const sec = Math.floor(seconds % 60);
    return `${min < 10 ? '0' : ''}${min}:${sec < 10 ? '0' : ''}${sec}`;
  };

  const handleSeek = (e: ChangeEvent<HTMLInputElement>) => {
    setCurrentTime(parseInt(e.target.value));
  };

  const togglePlay = () => {
    setIsPlaying(!isPlaying);
  };

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <div className="absolute inset-0 flex items-center justify-center z-50">
        {/* Dark blurred glass backdrop */}
        <motion.div
          id="video-player-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
          className="absolute inset-0 bg-black/90 backdrop-blur-md"
        />

        {/* Video Player Case (16:9 widescreen form layout) */}
        <motion.div
          id="video-player-case"
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="relative w-[780px] aspect-video bg-black border border-white/10 rounded-2xl overflow-hidden flex flex-col shadow-2xl"
        >
          {/* Upper Title Bar overlay on video */}
          <div className="absolute top-0 left-0 right-0 p-4 bg-gradient-to-b from-black/80 to-transparent flex items-center justify-between z-10 transition-opacity duration-300">
            <span className="text-white text-sm font-semibold tracking-wide drop-shadow-md">
              正在播放：{title} 官方影视预告
            </span>
            <button
              id="btn-close-video"
              onClick={onClose}
              className="w-8 h-8 rounded-full bg-black/40 hover:bg-black/70 border border-white/15 flex items-center justify-center text-[#fff] transition-colors cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Dummy Active Video Stage Screen */}
          <div className="flex-1 relative bg-[#0b0c10] flex items-center justify-center">
            {/* Poster image that pans or pulses slightly when playing */}
            <motion.img
              src={posterImage}
              alt="Video Poster"
              referrerPolicy="no-referrer"
              className="absolute inset-0 w-full h-full object-cover opacity-80"
              animate={isPlaying ? { scale: [1, 1.03, 1] } : {}}
              transition={{ duration: 15, repeat: Infinity, ease: 'linear' }}
            />

            {/* Glowing cyan/red fantasy overlay layer that flashes when video is simulated playing */}
            {isPlaying && (
              <div className="absolute inset-0 bg-gradient-to-br from-amber-500/10 via-transparent to-purple-500/15 animate-pulse mix-blend-color-dodge" />
            )}

            {/* Play/Pause giant icon that fades in when clicking screen */}
            <div 
              className="absolute inset-0 flex items-center justify-center cursor-pointer"
              onClick={togglePlay}
            >
              {!isPlaying && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.8 }}
                  animate={{ opacity: 1, scale: 1 }}
                  className="w-16 h-16 rounded-full bg-black/60 border border-white/25 flex items-center justify-center text-white"
                >
                  <Play className="w-7 h-7 fill-white translate-x-0.5" />
                </motion.div>
              )}
            </div>
          </div>

          {/* Interactive Player Controls Navigation bar at bottom */}
          <div className="bg-[#0f1015] border-t border-white/10 p-4 pt-2.5 flex flex-col space-y-2">
            {/* Progressive seek bar slider */}
            <div className="flex items-center space-x-3.5">
              <span className="text-[11px] font-mono text-gray-400">{formatTime(currentTime)}</span>
              <input
                type="range"
                min="0"
                max={duration}
                value={currentTime}
                onChange={handleSeek}
                className="w-full h-1 bg-white/15 rounded-lg appearance-none cursor-pointer accent-amber-400"
              />
              <span className="text-[11px] font-mono text-gray-400">{formatTime(duration)}</span>
            </div>

            {/* Sub-controls line */}
            <div className="flex items-center justify-between mt-1">
              <div className="flex items-center space-x-4">
                {/* Play / Pause Toggle button */}
                <button
                  onClick={togglePlay}
                  className="text-gray-300 hover:text-white transition-colors cursor-pointer"
                >
                  {isPlaying ? <Pause className="w-4 h-4 fill-white text-white" /> : <Play className="w-4 h-4 fill-white text-white" />}
                </button>

                {/* Restart Loop button */}
                <button
                  onClick={() => setCurrentTime(0)}
                  className="text-gray-400 hover:text-white transition-colors cursor-pointer"
                  title="重新播放"
                >
                  <RotateCcw className="w-4 h-4" />
                </button>

                {/* Volume slider control */}
                <div className="flex items-center space-x-2">
                  <Volume2 className="w-4 h-4 text-gray-400" />
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    value={volume}
                    onChange={(e) => setVolume(parseFloat(e.target.value))}
                    className="w-16 h-1 bg-white/15 rounded-sm appearance-none cursor-pointer accent-white"
                  />
                </div>
              </div>

              {/* simulated expand widescreen layout fullscreen toggle */}
              <button
                onClick={() => setIsPlaying((value) => !value)}
                className="text-gray-400 hover:text-white transition-colors cursor-pointer"
                title="全屏模式"
              >
                <Maximize className="w-4.5 h-4.5" />
              </button>
            </div>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
