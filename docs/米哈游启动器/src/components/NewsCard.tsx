import React, { useState } from 'react';
import { Play } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { GameNewsItem } from '../types';

interface NewsCardProps {
  pvCardImage: string;
  pvTitle: string;
  news: GameNewsItem[];
  accentColor: string;
  onOpenPV: () => void;
  onSelectNews: (news: GameNewsItem) => void;
}

type TabType = '活动' | '公告' | '资讯';

export default function NewsCard({
  pvCardImage,
  pvTitle,
  news,
  accentColor,
  onOpenPV,
  onSelectNews,
}: NewsCardProps) {
  const [activeTab, setActiveTab] = useState<TabType>('活动');

  const tabs: TabType[] = ['活动', '公告', '资讯'];

  const filteredNews = news.filter((item) => item.category === activeTab);

  const getAccentClass = () => {
    switch (accentColor) {
      case 'indigo':
        return 'bg-indigo-500 shadow-[0_0_8px_rgba(99,102,241,0.6)]';
      case 'yellow':
        return 'bg-[#fed330] shadow-[0_0_8px_rgba(254,211,48,0.6)]';
      default:
        return 'bg-amber-500 shadow-[0_0_8px_rgba(245,158,11,0.6)]';
    }
  };

  const getAccentTextClass = () => {
    switch (accentColor) {
      case 'indigo':
        return 'text-indigo-400';
      case 'yellow':
        return 'text-[#fed330]';
      default:
        return 'text-amber-400';
    }
  };

  return (
    <div className="bg-[#121319]/85 backdrop-blur-xl border border-white/10 rounded-2xl w-[325px] h-[310px] shadow-[0_12px_36px_rgba(0,0,0,0.6)] hover:border-white/15 transition-all duration-300 flex flex-col z-35 text-[14px] group overflow-hidden">
      
      {/* TOP SECTION: PV Trailer block (vertical upper half) */}
      <div 
        className="relative w-full h-[140px] overflow-hidden cursor-pointer shrink-0 border-b border-white/5 group/pv"
        onClick={onOpenPV}
      >
        <img
          src={pvCardImage}
          alt={pvTitle}
          referrerPolicy="no-referrer"
          className="w-full h-full object-cover group-hover/pv:scale-[1.05] transition-transform duration-[600ms] ease-out"
        />

        {/* Shadow Overlay */}
        <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/25 to-black/35 group-hover/pv:via-black/20 transition-all duration-300" />
        
        {/* Play Icon Circle (Centered beautifully) */}
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <motion.div
            whileHover={{ scale: 1.1 }}
            className="w-10 h-10 rounded-full bg-black/55 border border-white/10 backdrop-blur-sm flex items-center justify-center shadow-lg group-hover/pv:bg-[#fed330] group-hover/pv:border-[#fed330] transition-all duration-300 pointer-events-none"
          >
            <Play className="w-4.5 h-4.5 fill-current text-white group-hover/pv:text-[#121319] ml-0.5 transition-colors duration-200" />
          </motion.div>
        </div>

        {/* Banner Title Details bottom left */}
        <div className="absolute bottom-2 left-3 right-3 flex flex-col space-y-0.5 pointer-events-none text-left">
          <span className="text-[9px] uppercase tracking-wider text-[#fed330] font-black drop-shadow font-mono leading-none">
            官方精彩 PV
          </span>
          <span className="text-white font-semibold text-xs truncate drop-shadow-md leading-tight">
            {pvTitle}
          </span>
        </div>
      </div>

      {/* BOTTOM SECTION: Tabs segment & list (vertical lower half) */}
      <div className="flex-1 w-full flex flex-col p-3.5 pt-2.5 overflow-hidden text-left">
        
        {/* TABS line bar */}
        <div className="flex border-b border-white/5 space-x-1.5 pb-[2px] mb-2 shrink-0">
          {tabs.map((tab) => {
            const isActive = activeTab === tab;
            return (
              <button
                key={tab}
                id={`tab-${tab}`}
                onClick={() => setActiveTab(tab)}
                className={`relative px-3 pb-1.5 font-bold text-xs leading-none transition-all duration-200 cursor-pointer border-none bg-transparent focus:outline-none ${
                  isActive ? 'text-white' : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                {tab}
                {isActive && (
                  <motion.div
                    layoutId="activeTabUnderline"
                    className={`absolute bottom-[-3px] left-1.5 right-1.5 h-[2px] rounded-full ${getAccentClass()}`}
                    transition={{ type: 'spring', stiffness: 350, damping: 28 }}
                  />
                )}
              </button>
            );
          })}
        </div>

        {/* Text Announcements Area styled carefully (Ref: Fig 6) */}
        <div className="flex-1 overflow-y-auto pr-0.5 space-y-1.5 scrollbar-none">
          <AnimatePresence mode="popLayout">
            {filteredNews.length > 0 ? (
              filteredNews.map((item, index) => (
                <motion.div
                  key={item.title}
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={{ duration: 0.16, delay: index * 0.03 }}
                  onClick={() => onSelectNews(item)}
                  className="flex items-center justify-between py-1 px-1.5 rounded hover:bg-white/[0.03] transition-all duration-205 cursor-pointer border-l-2 border-transparent hover:border-white/10 group/item select-none text-left"
                >
                  {/* Tag + title inline */}
                  <div className="flex items-center space-x-1 min-w-0 flex-1">
                    <span className={`text-[12px] font-bold shrink-0 ${getAccentTextClass()} select-none`}>
                      【{item.category}】
                    </span>
                    <span className="text-gray-300 group-hover/item:text-white transition-colors duration-150 truncate text-[12px] font-medium leading-none no-underline">
                      {item.title}
                    </span>
                  </div>

                  {/* Date format */}
                  <span className="text-[10.5px] text-gray-500 font-mono tracking-tight group-hover/item:text-gray-400 transition-colors duration-150 font-light shrink-0 pl-2">
                    {item.date}
                  </span>
                </motion.div>
              ))
            ) : (
              <div className="flex flex-col items-center justify-center h-full text-[11px] text-gray-500 space-y-1 py-4 select-none">
                <span>暂无相关的公告资讯</span>
              </div>
            )}
          </AnimatePresence>
        </div>
      </div>

    </div>
  );
}
