import React from 'react';
import { motion } from 'motion/react';
import { Globe, Github } from 'lucide-react';
import { SocialLink } from '../types';

interface SocialSidebarProps {
  socials: SocialLink[];
  onSocialClick: (link: SocialLink) => void;
}

export default function SocialSidebar({ socials, onSocialClick }: SocialSidebarProps) {
  // Render high-fidelity SVGs styled exactly to size 24px and optimized for high-res look
  const renderIcon = (iconName: string) => {
    switch (iconName) {
      case 'QQ':
        return (
          <svg viewBox="0 0 24 24" className="w-[24px] h-[24px] fill-current text-white/80 group-hover:text-white transition-colors">
            {/* High-fidelity Penguin shape matching hover state exactly */}
            <path d="M12 3C8.686 3 6 5.686 6 9C6 11.23 7.025 12.645 8.01 13.91C8.42 14.44 8.219 14.897 7.78 15.34C6.46 16.68 5.5 18 5.5 19.5C5.5 21 7 21.5 12 21.5C17 21.5 18.5 21 18.5 19.5C18.5 18 17.54 16.68 16.22 15.34C15.78 14.897 15.576 14.44 15.987 13.91C16.97 12.645 18 11.23 18 9C18 5.686 15.313 3 12 3Z" fill="currentColor" fillOpacity="0.15" />
            <path d="M12 3C8.686 3 6 5.686 6 9C6 11.23 7.025 12.645 8.01 13.91C8.42 14.44 8.219 14.897 7.78 15.34C6.46 16.68 5.5 18 5.5 19.5C5.5 21 7 21.5 12 21.5C17 21.5 18.5 21 18.5 19.5C18.5 18 17.54 16.68 16.22 15.34C15.78 14.897 15.576 14.44 15.987 13.91C16.97 12.645 18 11.23 18 9C18 5.686 15.313 3 12 3Z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" fill="none" />
            <path d="M8.5 16.5C10 17 14 17 15.5 16.5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" fill="none" />
          </svg>
        );
      case 'Discord':
        return (
          <svg viewBox="0 0 127.14 96.36" className="w-[23px] h-[23px] fill-current text-white/80 group-hover:text-white transition-colors">
            {/* Highly polished authentic Discord vector path */}
            <path d="M107.7,8.07A105.15,105.15,0,0,0,77.26,0a77.19,77.19,0,0,0-3.3,6.83A96.67,96.67,0,0,0,53.22,6.83,77.19,77.19,0,0,0,49.88,0,105.15,105.15,0,0,0,19.44,8.07C3.66,31.58-1.86,54.65,1,77.53A105.73,105.73,0,0,0,32,96.36a77.7,77.7,0,0,0,6.63-10.85,68.43,68.43,0,0,1-10.5-5c.88-.65,1.72-1.34,2.51-2.07a75.43,75.43,0,0,0,73,0c.79.73,1.63,1.42,2.51,2.07a68.43,68.43,0,0,1-10.5,5A77.7,77.7,0,0,0,95.14,96.36a105.73,105.73,0,0,0,31-18.83C129,54.65,123.5,31.58,107.7,8.07ZM42.45,65.69C36.18,65.69,31,60,31,53S36.18,40.36,42.45,40.36,53.83,46,53.83,53,48.72,65.69,42.45,65.69Zm42.24,0C78.41,65.69,73.24,60,73.24,53S78.41,40.36,84.69,40.36,96.07,46,96.07,53,91,65.69,84.69,65.69Z" />
          </svg>
        );
      case 'Globe':
        return (
          <Globe className="w-[23px] h-[23px] text-white/80 group-hover:text-white transition-colors" strokeWidth={1.8} />
        );
      case 'Github':
        return (
          <Github className="w-[23px] h-[23px] text-white/80 group-hover:text-white transition-colors" />
        );
      default:
        return (
          <Globe className="w-[23px] h-[23px] text-white/80 group-hover:text-white transition-colors" strokeWidth={1.8} />
        );
    }
  };

  return (
    <div className="flex flex-col space-y-[15px] items-center justify-center">
      {socials.map((social, index) => (
        <div key={social.name} className="relative group flex items-center justify-center">
          {/* Hover Tooltip (Pops up gracefully on the left, resembling HoyoPlay's original banner popup) */}
          <div className="absolute right-[44px] scale-90 translate-x-2 opacity-0 group-hover:scale-100 group-hover:translate-x-0 group-hover:opacity-100 pointer-events-none transition-all duration-200 z-50 flex items-center shadow-xl">
            <div className="bg-[#1b1c21]/95 backdrop-blur-md text-white text-[12px] font-bold pl-4 pr-3 py-2.5 rounded-lg border border-white/5 shadow-2x flex items-center space-x-4 whitespace-nowrap">
              <span className="tracking-wide">
                {social.name === 'QQ' ? '加入官方QQ群' : 
                 social.name === 'Discord' ? '加入 Discord 社区' : 
                 social.name === '官网' ? '访问官方网站' : 
                 social.name === 'GitHub' ? '访问 GitHub 开源仓库' : social.tooltip}
              </span>
              <span className="text-white/40 text-[9px] font-bold font-sans">＞</span>
            </div>
          </div>

          {/* Clean hover-only frame to simulate HoyoPlay standard button states */}
          <motion.a
            id={`social-launcher-${index}`}
            href={social.url}
            target="_blank"
            rel="noreferrer"
            whileTap={{ scale: 0.92 }}
            className="w-10 h-10 flex items-center justify-center rounded-lg bg-transparent hover:bg-white/10 text-white/70 hover:text-white transition-all cursor-pointer focus:outline-none select-none"
            onClick={(e) => {
              if (social.url === '#' || social.url.startsWith('#')) {
                e.preventDefault();
                onSocialClick(social);
              }
            }}
          >
            {renderIcon(social.icon)}
          </motion.a>
        </div>
      ))}
    </div>
  );
}
