export interface GameNewsItem {
  title: string;
  date: string;
  category: '活动' | '公告' | '资讯';
}

export interface SocialLink {
  name: string;
  icon: string;
  tooltip: string;
  url: string;
}

export interface GameConfig {
  id: string;
  name: string;
  enName: string;
  logoText: string;
  logoSubtext: string;
  bannerCategory: string;
  bannerTitle: string;
  bannerSubtitle: string;
  bannerButtonText: string;
  bgImage: string;
  pvCardImage: string;
  pvTitle: string;
  accentColor: string; // e.g., "yellow" or "sky"
  news: GameNewsItem[];
  sizeGB: number;
  installSpeedMB: number;
  socials: SocialLink[];
}
