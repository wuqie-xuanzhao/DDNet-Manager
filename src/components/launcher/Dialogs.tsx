import { AnimatePresence, motion } from 'framer-motion';
import { X, AlertTriangle, MessageSquare, Heart, Eye } from 'lucide-react';
import { useState } from 'react';
import type { GameNewsItem } from './types';

// --- 1. POST DETAIL MODAL ---
interface PostDetailModalProps {
  isOpen: boolean;
  onClose: () => void;
  post: GameNewsItem | null;
  gameName: string;
}

export function PostDetailModal({ isOpen, onClose, post, gameName }: PostDetailModalProps) {
  if (!post) return null;

  return (
    <AnimatePresence>
      {isOpen && (
        <div className="absolute inset-0 flex items-center justify-center z-50">
          <motion.div
            id="post-modal-backdrop"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="absolute inset-0 bg-black/75 backdrop-blur-md"
          />

          <motion.div
            id="post-modal-content"
            initial={{ opacity: 0, scale: 0.9, y: 30 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.9, y: 30 }}
            className="relative w-[560px] max-h-[85%] bg-[#1a1c24] border border-white/10 rounded-2xl overflow-hidden flex flex-col shadow-2xl"
          >
            {/* Header banner background */}
            <div className="bg-gradient-to-r from-amber-500/20 to-purple-500/20 px-6 py-4 border-b border-white/10 flex items-center justify-between">
              <span className="text-amber-400 text-xs font-bold font-mono uppercase tracking-widest">
                {gameName} · {post.category}
              </span>
              <button
                id="btn-close-post"
                onClick={onClose}
                className="w-7 h-7 rounded-full bg-white/5 hover:bg-white/10 flex items-center justify-center text-gray-400 hover:text-white transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Scrollable Body */}
            <div className="p-6 overflow-y-auto space-y-5 text-sm leading-relaxed text-gray-300">
              <h2 className="text-xl font-bold text-white tracking-wide leading-snug">
                {post.title}
              </h2>

              <div className="flex items-center space-x-4 border-y border-white/5 py-2.5 text-xs text-gray-500 font-mono">
                <span>发布日期: 2026-{post.date}</span>
                <span>来源: DDNet Manager</span>
                <span className="flex items-center">
                  <Eye className="w-3.5 h-3.5 mr-1" />
                  {gameName}
                </span>
              </div>

              {/* Content Body */}
              <div className="space-y-4">
                <p>
                  {post.title}
                </p>
                <p className="text-gray-400 text-xs">
                  详情待更新。
                </p>
              </div>

              {/* Footer */}
              <div className="flex items-center justify-between border-t border-white/5 pt-5 mt-6 text-xs text-gray-400">
                <div className="flex space-x-3">
                  <button className="flex items-center space-x-1 hover:text-red-400 transition-colors p-1">
                    <Heart className="w-4 h-4 text-red-400 fill-red-400" />
                    <span>{gameName}</span>
                  </button>
                  <button className="flex items-center space-x-1 hover:text-amber-400 transition-colors p-1">
                    <MessageSquare className="w-4 h-4" />
                    <span>{post.category}</span>
                  </button>
                </div>
                <span>DDNet Manager</span>
              </div>
            </div>

            {/* Sticky footer button */}
            <div className="p-4 bg-white/5 border-t border-white/10 flex justify-end">
              <button
                id="btn-post-ok"
                onClick={onClose}
                className="px-5 py-2 rounded-lg bg-amber-400 hover:bg-amber-300 text-black text-xs font-semibold cursor-pointer transition-colors"
              >
                已阅并返回
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}


// --- 2. CONFIRM EXIT LAUNCHER DIALOG ---
interface ConfirmExitModalProps {
  isOpen: boolean;
  onCancel: () => void;
  onConfirmExit: () => void;
  onMinimize: () => void;
}

export function ConfirmExitModal({ isOpen, onCancel, onConfirmExit, onMinimize }: ConfirmExitModalProps) {
  const [option, setOption] = useState<'minimize' | 'quit'>('minimize');

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <div className="absolute inset-0 isolate flex items-center justify-center z-50">
        <motion.div
          id="exit-modal-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onCancel}
          className="absolute inset-0 z-0 bg-black/80 backdrop-blur-md"
        />

        <motion.div
          id="exit-modal-content"
          initial={{ opacity: 0, scale: 0.9, y: 30 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.9, y: 30 }}
          className="relative z-10 w-[440px] bg-[#161821] border border-white/10 rounded-2xl shadow-2xl p-6 flex flex-col space-y-5"
        >
          {/* Header Warning */}
          <div className="flex items-center space-x-2.5">
            <AlertTriangle className="w-5 h-5 text-amber-400" />
            <span className="text-white text-base font-bold tracking-wide">
              关闭启动器
            </span>
          </div>

          <div className="flex flex-col space-y-2.5">
            <button
              id="exit-opt-minimize"
              onClick={() => setOption('minimize')}
              className={`p-3 rounded-xl border text-left flex items-center space-x-3.5 transition-all text-xs cursor-pointer ${
                option === 'minimize'
                  ? 'bg-amber-400/10 border-amber-400/40 text-white'
                  : 'bg-white/5 border-transparent text-gray-400 hover:bg-white/10'
              }`}
            >
              <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0 ${
                option === 'minimize' ? 'border-amber-400' : 'border-gray-500'
              }`}>
                {option === 'minimize' && <div className="w-2 h-2 rounded-full bg-amber-400" />}
              </div>
              <div>
                <span className="font-semibold block">最小化</span>
              </div>
            </button>

            <button
              id="exit-opt-quit"
              onClick={() => setOption('quit')}
              className={`p-3 rounded-xl border text-left flex items-center space-x-3.5 transition-all text-xs cursor-pointer ${
                option === 'quit'
                  ? 'bg-red-400/10 border-red-400/40 text-white'
                  : 'bg-white/5 border-transparent text-gray-400 hover:bg-white/10'
              }`}
            >
              <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0 ${
                option === 'quit' ? 'border-red-400' : 'border-gray-500'
              }`}>
                {option === 'quit' && <div className="w-2 h-2 rounded-full bg-red-400" />}
              </div>
              <div>
                <span className="font-semibold block">退出</span>
              </div>
            </button>
          </div>

          {/* Actions Bottom Bar */}
          <div className="flex justify-end space-x-3 pt-2">
            <button
              onClick={onCancel}
              className="px-5 py-2.5 rounded-lg bg-white/5 hover:bg-white/10 text-gray-400 hover:text-white text-xs font-semibold cursor-pointer transition-colors"
            >
              取 消
            </button>
            <button
              id="btn-confirm-exit-act"
              onClick={() => {
                if (option === 'minimize') {
                  onMinimize();
                } else {
                  onConfirmExit();
                }
              }}
              className="px-6 py-2.5 rounded-lg bg-amber-400 hover:bg-amber-300 text-black text-xs font-bold cursor-pointer transition-colors"
            >
              确 认
            </button>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
