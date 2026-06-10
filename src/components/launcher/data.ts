import genshinBg from "@/assets/launcher/genshin_natlan_bg_1780939559326.png";
import genshinCard from "@/assets/launcher/genshin_pv_card_1780939604512.png";
import starRailBg from "@/assets/launcher/star_rail_blade_bg_1780939473544.png";
import starRailCard from "@/assets/launcher/star_rail_pv_card_1780939496529.png";
import zzzBg from "@/assets/launcher/zzz_eridu_bg_1780939579833.png";
import zzzCard from "@/assets/launcher/zzz_pv_card_1780939623168.png";
import type { GameConfig } from "./types";

export type LauncherGameId = "qmclient" | "ddnet" | "ddnet-steam" | "third-party";

export const GAME_ICON_MAP: Record<LauncherGameId, string> = {
  qmclient: "/src/assets/logo.svg",
  ddnet: "/src/assets/ddnet2.webp",
  "ddnet-steam": "/src/assets/ddnet2.webp",
  "third-party": "/src/assets/logo.svg"
};

export const GAMES_DATA: GameConfig[] = [
  {
    id: "qmclient",
    name: "QmClient",
    enName: "QmClient",
    logoText: "DDNet Manager",
    logoSubtext: "DDNET MANAGER",
    bannerCategory: "主力客户端",
    bannerTitle: "QmClient",
    bannerSubtitle: "默认客户端 / 更新检查 / 本地注册表集中管理",
    bannerButtonText: "查看更新",
    bgImage: starRailBg,
    pvCardImage: starRailCard,
    pvTitle: "QmClient 客户端状态",
    accentColor: "amber",
    sizeGB: 0.9,
    installSpeedMB: 84.5,
    news: [
      { title: "QmClient 下载、校验和安装事务接入默认流程", date: "06/09", category: "公告" },
      { title: "本地客户端目录验证与候选扫描持续优化", date: "06/08", category: "资讯" },
      { title: "默认客户端启动前会复检可执行文件与运行状态", date: "06/07", category: "活动" }
    ],
    socials: [
      { name: "QQ", icon: "QQ", tooltip: "加入 QmClient QQ 群", url: "mqqapi://card/show_pslcard?src_type=internal&version=1&uin=1076765929&card_type=group&source=qrcode" },
      { name: "Workshop", icon: "Globe", tooltip: "访问 DDRace Workshop", url: "https://ddrace.cn/" },
      { name: "DDNet", icon: "Globe", tooltip: "访问 DDNet 官方网站", url: "https://ddnet.org/" },
      { name: "GitHub", icon: "Github", tooltip: "访问项目仓库", url: "https://github.com/ddnet/ddnet" }
    ]
  },
  {
    id: "ddnet",
    name: "DDNet",
    enName: "DDrace Network",
    logoText: "DDNet",
    logoSubtext: "VANILLA CLIENT",
    bannerCategory: "官方客户端",
    bannerTitle: "官方发行版",
    bannerSubtitle: "原版体验 / 官方渠道 / 统一启动",
    bannerButtonText: "打开官网",
    bgImage: genshinBg,
    pvCardImage: genshinCard,
    pvTitle: "DDNet 官方客户端",
    accentColor: "indigo",
    sizeGB: 0.5,
    installSpeedMB: 93.2,
    news: [
      { title: "官方客户端可通过目录扫描纳入统一管理", date: "06/08", category: "公告" },
      { title: "启动器保留多第三方客户端扩展模型", date: "06/07", category: "资讯" },
      { title: "更新源默认不把开发仓库 Release 目录作为用户源", date: "06/06", category: "活动" }
    ],
    socials: [
      { name: "DDNet", icon: "Globe", tooltip: "访问 DDNet 官方网站", url: "https://ddnet.org/" },
      { name: "Workshop", icon: "Globe", tooltip: "访问 DDRace Workshop", url: "https://ddrace.cn/" },
      { name: "GitHub", icon: "Github", tooltip: "访问 DDNet GitHub", url: "https://github.com/ddnet/ddnet" }
    ]
  },
  {
    id: "ddnet-steam",
    name: "DDNet Steam",
    enName: "Steam Edition",
    logoText: "Steam",
    logoSubtext: "DDNET",
    bannerCategory: "Steam 安装",
    bannerTitle: "Steam 版",
    bannerSubtitle: "扫描 steamapps/common/DDNet 并保存默认启动项",
    bannerButtonText: "扫描目录",
    bgImage: zzzBg,
    pvCardImage: zzzCard,
    pvTitle: "Steam 安装识别",
    accentColor: "yellow",
    sizeGB: 0.6,
    installSpeedMB: 75.1,
    news: [
      { title: "Steam 安装路径会被识别为独立客户端类型", date: "06/09", category: "活动" },
      { title: "默认客户端选择已持久化到本地注册表", date: "06/08", category: "公告" },
      { title: "运行状态检测用于避免重复启动", date: "06/07", category: "资讯" }
    ],
    socials: [
      { name: "Steam", icon: "Globe", tooltip: "打开 Steam 商店页", url: "https://store.steampowered.com/app/412220/DDNet/" },
      { name: "DDNet", icon: "Globe", tooltip: "访问 DDNet 官方网站", url: "https://ddnet.org/" },
      { name: "GitHub", icon: "Github", tooltip: "访问 DDNet GitHub", url: "https://github.com/ddnet/ddnet" }
    ]
  },
  {
    id: "third-party",
    name: "第三方客户端",
    enName: "Third-party Clients",
    logoText: "Clients",
    logoSubtext: "CUSTOM",
    bannerCategory: "兼容客户端",
    bannerTitle: "兼容客户端",
    bannerSubtitle: "TaterClient / BestClient / 自定义客户端",
    bannerButtonText: "管理客户端",
    bgImage: starRailBg,
    pvCardImage: starRailCard,
    pvTitle: "兼容客户端管理",
    accentColor: "indigo",
    sizeGB: 0.7,
    installSpeedMB: 65.4,
    news: [
      { title: "第三方客户端可通过手动路径加入管理", date: "06/08", category: "活动" },
      { title: "客户端模型保留安装来源、版本与可执行路径", date: "06/07", category: "公告" },
      { title: "后续更新能力按客户端 catalog 分派来源", date: "06/06", category: "资讯" }
    ],
    socials: [
      { name: "QQ", icon: "QQ", tooltip: "加入 QmClient QQ 群", url: "mqqapi://card/show_pslcard?src_type=internal&version=1&uin=1076765929&card_type=group&source=qrcode" },
      { name: "Workshop", icon: "Globe", tooltip: "访问 DDRace Workshop", url: "https://ddrace.cn/" },
      { name: "GitHub", icon: "Github", tooltip: "访问 DDNet GitHub", url: "https://github.com/ddnet/ddnet" }
    ]
  }
];
