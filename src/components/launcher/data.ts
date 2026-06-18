import genshinBg from "@/assets/launcher/genshin_natlan_bg_1780939559326.png";
import genshinCard from "@/assets/launcher/genshin_pv_card_1780939604512.png";
import starRailBg from "@/assets/launcher/star_rail_blade_bg_1780939473544.png";
import starRailCard from "@/assets/launcher/star_rail_pv_card_1780939496529.png";
import zzzBg from "@/assets/launcher/zzz_eridu_bg_1780939579833.png";
import zzzCard from "@/assets/launcher/zzz_pv_card_1780939623168.png";
import type { ClientCatalogEntry } from "@/types";
import type { GameConfig } from "./types";

/// 启动器里出现的 game id。DDNet 同时识别 Steam 安装和官网下载版（合并成一个 tab）。
/// `third_party` 仅作为 client_id 用于识别兜底（infer_client_identity 内部），
/// 不在 gallery 显示。
export type LauncherGameId = "ddnet" | "qmclient" | "taterclient" | "bestclient" | "cactusclient";

export const GAME_ICON_MAP: Record<string, string> = {
  ddnet: "/src/assets/ddnet2.webp",
  qmclient: "/src/assets/logo.svg",
  taterclient: "/src/assets/logo.svg",
  bestclient: "/src/assets/logo.svg",
  cactusclient: "/src/assets/logo.svg"
};

/// 按 client_id 映射的纯视觉资产（背景图、PV 卡、accent 色、文案、新闻、社交链接）。
/// 业务字段（client_id、display_name、upstream_url、更新源、PE 元信息）由 Rust catalog 提供，
/// 运行时合并成 GameConfig。
type ClientVisualAssets = Omit<GameConfig, "id" | "name" | "enName">;

const QMCLIENT_SOCIALS = [
  { name: "QQ", icon: "QQ", tooltip: "加入 QmClient QQ 群", url: "mqqapi://card/show_pslcard?src_type=internal&version=1&uin=1076765929&card_type=group&source=qrcode" },
  { name: "Workshop", icon: "Globe", tooltip: "访问 DDRace Workshop", url: "https://ddrace.cn/" },
  { name: "DDNet", icon: "Globe", tooltip: "访问 DDNet 官方网站", url: "https://ddnet.org/" },
  { name: "GitHub", icon: "Github", tooltip: "访问项目仓库", url: "https://github.com/ddnet/ddnet" }
];

const DDNET_SOCIALS = [
  { name: "DDNet", icon: "Globe", tooltip: "访问 DDNet 官方网站", url: "https://ddnet.org/" },
  { name: "Workshop", icon: "Globe", tooltip: "访问 DDRace Workshop", url: "https://ddrace.cn/" },
  { name: "Steam", icon: "Globe", tooltip: "打开 Steam 商店页", url: "https://store.steampowered.com/app/412220/DDNet/" },
  { name: "GitHub", icon: "Github", tooltip: "访问 DDNet GitHub", url: "https://github.com/ddnet/ddnet" }
];

const VISUAL_ASSETS: Record<string, ClientVisualAssets> = {
  ddnet: {
    logoText: "DDNet",
    logoSubtext: "OFFICIAL CLIENT",
    bannerCategory: "官方客户端",
    bannerTitle: "官方发行版",
    bannerSubtitle: "DDNet 官方客户端（Steam / 官网下载共用）",
    bannerButtonText: "打开官网",
    bgImage: genshinBg,
    pvCardImage: genshinCard,
    pvTitle: "DDNet 官方客户端",
    accentColor: "indigo",
    sizeGB: 0.5,
    installSpeedMB: 93.2,
    news: [
      { title: "目录扫描", date: "06/08", category: "公告" },
      { title: "扩展模型", date: "06/07", category: "资讯" },
      { title: "更新源", date: "06/06", category: "活动" }
    ],
    socials: DDNET_SOCIALS
  },
  qmclient: {
    logoText: "DDNet Manager",
    logoSubtext: "DDNET MANAGER",
    bannerCategory: "已安装",
    bannerTitle: "QmClient",
    bannerSubtitle: "社区主流客户端",
    bannerButtonText: "查看更新",
    bgImage: starRailBg,
    pvCardImage: starRailCard,
    pvTitle: "QmClient 客户端状态",
    accentColor: "amber",
    sizeGB: 0.9,
    installSpeedMB: 84.5,
    news: [
      { title: "下载、校验、安装", date: "06/09", category: "公告" },
      { title: "目录验证", date: "06/08", category: "资讯" },
      { title: "版本检查", date: "06/07", category: "活动" }
    ],
    socials: QMCLIENT_SOCIALS
  },
  taterclient: {
    logoText: "TaterClient",
    logoSubtext: "TATER",
    bannerCategory: "社区客户端",
    bannerTitle: "TaterClient",
    bannerSubtitle: "TaterClient 社区版",
    bannerButtonText: "查看更新",
    bgImage: starRailBg,
    pvCardImage: starRailCard,
    pvTitle: "TaterClient 客户端状态",
    accentColor: "amber",
    sizeGB: 0.85,
    installSpeedMB: 80.0,
    news: [
      { title: "GitHub Release", date: "06/09", category: "公告" },
      { title: "客户端识别", date: "06/08", category: "资讯" }
    ],
    socials: QMCLIENT_SOCIALS
  },
  bestclient: {
    logoText: "BestClient",
    logoSubtext: "BEST",
    bannerCategory: "社区客户端",
    bannerTitle: "BestClient",
    bannerSubtitle: "BestClient 社区版",
    bannerButtonText: "查看更新",
    bgImage: zzzBg,
    pvCardImage: zzzCard,
    pvTitle: "BestClient 客户端状态",
    accentColor: "yellow",
    sizeGB: 0.8,
    installSpeedMB: 75.0,
    news: [
      { title: "GitHub Release", date: "06/09", category: "公告" }
    ],
    socials: QMCLIENT_SOCIALS
  },
  cactusclient: {
    logoText: "Cactus",
    logoSubtext: "CACTUS",
    bannerCategory: "社区客户端",
    bannerTitle: "Cactus Client",
    bannerSubtitle: "仙人掌客户端",
    bannerButtonText: "访问官网",
    bgImage: zzzBg,
    pvCardImage: zzzCard,
    pvTitle: "Cactus Client 状态",
    accentColor: "yellow",
    sizeGB: 0.75,
    installSpeedMB: 70.0,
    news: [
      { title: "官网发布", date: "06/09", category: "公告" }
    ],
    socials: QMCLIENT_SOCIALS
  }
};

const DEFAULT_VISUAL: ClientVisualAssets = VISUAL_ASSETS.ddnet;

/// 把 Rust catalog 业务数据 + 前端视觉资产合并成 GameConfig[]。
/// catalog 顺序就是 gallery 顺序（Rust 端已确保 DDNet 第一）。
/// third_party 不在 CATALOG 里，自然不会出现在 gallery。
export function buildGamesData(catalog: ClientCatalogEntry[]): GameConfig[] {
  return catalog.map((entry) => {
    const visual = VISUAL_ASSETS[entry.client_id] ?? DEFAULT_VISUAL;
    return {
      id: entry.client_id,
      name: entry.display_name,
      enName: entry.display_name,
      ...visual
    };
  });
}

/// 静态 fallback：浏览器预览 / catalog 未拉到 / tauriRuntime=false 时用。
/// 顺序与 Rust CATALOG 一致：DDNet / QmClient / TaterClient / BestClient / CactusClient。
export const GAMES_DATA: GameConfig[] = buildGamesData([
  {
    client_id: "ddnet",
    display_name: "DDNet",
    aliases: ["ddnet"],
    executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
    required_markers: ["storage.cfg", "data"],
    pe_company_names: [],
    pe_product_names: [],
    known_hashes: [],
    update_source: { kind: "ddnet_official" },
    upstream_url: "https://ddnet.org/downloads/"
  },
  {
    client_id: "qmclient",
    display_name: "QmClient",
    aliases: ["qmclient"],
    executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
    required_markers: ["storage.cfg", "data"],
    pe_company_names: [],
    pe_product_names: [],
    known_hashes: [],
    update_source: { kind: "github_release", owner: "wxj881027", repo: "QmClient", windows_assets: [], macos_assets: [], linux_assets: [] },
    upstream_url: "https://github.com/wxj881027/QmClient/releases"
  },
  {
    client_id: "taterclient",
    display_name: "TaterClient",
    aliases: ["taterclient"],
    executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
    required_markers: ["storage.cfg", "data"],
    pe_company_names: [],
    pe_product_names: [],
    known_hashes: [],
    update_source: { kind: "github_release", owner: "TaterClient", repo: "TClient", windows_assets: [], macos_assets: [], linux_assets: [] },
    upstream_url: "https://github.com/TaterClient/TClient/releases"
  },
  {
    client_id: "bestclient",
    display_name: "BestClient",
    aliases: ["bestclient"],
    executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
    required_markers: ["storage.cfg", "data"],
    pe_company_names: [],
    pe_product_names: [],
    known_hashes: [],
    update_source: { kind: "github_release", owner: "BestProjectTeam", repo: "BestClient", windows_assets: [], macos_assets: [], linux_assets: [] },
    upstream_url: "https://github.com/BestProjectTeam/BestClient/releases"
  },
  {
    client_id: "cactusclient",
    display_name: "Cactus Client",
    aliases: ["cactusclient"],
    executable_candidates: { windows: ["DDNet.exe"], macos: ["DDNet"], linux: ["DDNet"] },
    required_markers: ["storage.cfg", "data"],
    pe_company_names: [],
    pe_product_names: [],
    known_hashes: [],
    update_source: { kind: "website", url: "https://cactusss.vercel.app/" },
    upstream_url: "https://cactusss.vercel.app/"
  }
]);
