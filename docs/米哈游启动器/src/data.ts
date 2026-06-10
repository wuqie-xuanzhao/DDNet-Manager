import { GameConfig } from './types';

export const GAMES_DATA: GameConfig[] = [
  {
    id: 'star-rail',
    name: '崩坏：星穹铁道',
    enName: 'Honkai: Star Rail',
    logoText: '崩坏',
    logoSubtext: '星穹铁道',
    bannerCategory: '角色活动跃迁 ❖',
    bannerTitle: '我身即刃',
    bannerSubtitle: '4.3版本更新后 - 2026/06/24 11:59',
    bannerButtonText: '查看活动',
    bgImage: '/src/assets/images/star_rail_blade_bg_1780939473544.png',
    pvCardImage: '/src/assets/images/star_rail_pv_card_1780939496529.png',
    pvTitle: '千冶·刃角色PV',
    accentColor: 'amber',
    sizeGB: 76.5,
    installSpeedMB: 84.5,
    news: [
      {
        title: '沉于生者的忘川 | 4.3版本攻略征集活动',
        date: '06/01',
        category: '活动',
      },
      {
        title: '「千冶•刃」角色绘画征集活动开启！',
        date: '05/29',
        category: '活动',
      },
      {
        title: '「崩坏世界论坛」米游社专属签到福利升级公告',
        date: '06/02',
        category: '公告',
      },
      {
        title: '「差分宇宙」版本更新与机制优化细节一览',
        date: '05/30',
        category: '公告',
      },
      {
        title: '1.2「仙舟『罗浮』」线下主题快闪与演出现场情报公开',
        date: '05/28',
        category: '资讯',
      },
      {
        title: '《崩坏：星穹铁道》原创国风电音专辑上线网易云音乐',
        date: '05/27',
        category: '资讯',
      },
    ],
    socials: [
      { name: 'QQ', icon: 'QQ', tooltip: '加入官方QQ交流群', url: 'https://im.qq.com/' },
      { name: 'Discord', icon: 'Discord', tooltip: '加入官方 Discord 社区', url: 'https://discord.gg/honkaistarrail' },
      { name: '官网', icon: 'Globe', tooltip: '访问游戏官方网站', url: 'https://hsr.hoyoverse.com/' },
      { name: 'GitHub', icon: 'Github', tooltip: '查看 GitHub 开源仓库', url: 'https://github.com/hoyoverse' }
    ]
  },
  {
    id: 'genshin-impact',
    name: '原神',
    enName: 'Genshin Impact',
    logoText: '原神',
    logoSubtext: 'GENSHIN IMPACT',
    bannerCategory: '限定角色祈愿 ❖',
    bannerTitle: '金丝与丝绒的圆舞',
    bannerSubtitle: '5.2版本更新后 - 2026/07/02 17:59',
    bannerButtonText: '查看详情',
    bgImage: '/src/assets/images/genshin_natlan_bg_1780939559326.png',
    pvCardImage: '/src/assets/images/genshin_pv_card_1780939604512.png',
    pvTitle: '「炽羽凌霜」新角色演示',
    accentColor: 'indigo',
    sizeGB: 92.4,
    installSpeedMB: 93.2,
    news: [
      {
        title: '「纳塔游记」纳塔全新区域探索与解密夺宝赢原石',
        date: '06/05',
        category: '活动',
      },
      {
        title: '「神铸赋形」限定五星武器祈愿概率提升！',
        date: '06/03',
        category: '活动',
      },
      {
        title: '5.2版本「灵火燃耀之盛典」更新停服及补偿说明',
        date: '06/02',
        category: '公告',
      },
      {
        title: '非正常游戏账号冻结处罚公告（持续更新）',
        date: '05/31',
        category: '公告',
      },
      {
        title: '原神原画集 Vol.3 现已开启预售，限时附赠独家特典',
        date: '05/29',
        category: '资讯',
      },
      {
        title: '「寻味纳塔」主题全国餐饮联动店及专属套餐一览',
        date: '05/27',
        category: '资讯',
      },
    ],
    socials: [
      { name: 'QQ', icon: 'QQ', tooltip: '加入官方QQ交流群', url: 'https://im.qq.com/' },
      { name: 'Discord', icon: 'Discord', tooltip: '加入官方 Discord 社区', url: 'https://discord.gg/genshinimpact' },
      { name: '官网', icon: 'Globe', tooltip: '访问原神官方网站', url: 'https://ys.mihoyo.com/' },
      { name: 'GitHub', icon: 'Github', tooltip: '查看 GitHub 开源仓库', url: 'https://github.com/hoyoverse' }
    ]
  },
  {
    id: 'zenless-zone-zero',
    name: '绝区零',
    enName: 'Zenless Zone Zero',
    logoText: '绝区零',
    logoSubtext: 'ZENLESS ZONE ZERO',
    bannerCategory: '独家频段调频 ❖',
    bannerTitle: '高街奇才的黑胶巡礼',
    bannerSubtitle: '1.4全新版本「新艾利都的狂想」现已开启',
    bannerButtonText: '去到六分街',
    bgImage: '/src/assets/images/zzz_eridu_bg_1780939579833.png',
    pvCardImage: '/src/assets/images/zzz_pv_card_1780939623168.png',
    pvTitle: '「雅·虚空一闪」阵营PV',
    accentColor: 'yellow',
    sizeGB: 54.8,
    installSpeedMB: 75.1,
    news: [
      {
        title: '「沙罗黄金周」特别联络计划！参与即送30抽',
        date: '06/08',
        category: '活动',
      },
      {
        title: '「邦布大骚动」限时玩法开启，获取专属邦布券',
        date: '06/04',
        category: '活动',
      },
      {
        title: '1.4版本问题修复与游玩体验阶段性调优说明',
        date: '06/02',
        category: '公告',
      },
      {
        title: '新艾利都治安局关于「空洞异常行为」安全教育公告',
        date: '05/30',
        category: '公告',
      },
      {
        title: '全新实物周边：代理人毛绒公仔、亚克力摆件上架预售',
        date: '05/28',
        category: '资讯',
      },
      {
        title: '绝区零 1.4 OST《六分街的晨曦》现已全球上线',
        date: '05/26',
        category: '资讯',
      },
    ],
    socials: [
      { name: 'QQ', icon: 'QQ', tooltip: '加入官方QQ交流群', url: 'https://im.qq.com/' },
      { name: 'Discord', icon: 'Discord', tooltip: '加入官方 Discord 社区', url: 'https://discord.gg/zenlesszonezero' },
      { name: '官网', icon: 'Globe', tooltip: '访问绝区零官方网站', url: 'https://zzz.hoyoverse.com/' },
      { name: 'GitHub', icon: 'Github', tooltip: '查看 GitHub 开源仓库', url: 'https://github.com/hoyoverse' }
    ]
  },
  {
    id: 'honkai-3rd',
    name: '崩坏3',
    enName: 'Honkai Impact 3rd',
    logoText: '崩坏3',
    logoSubtext: 'HONKAI IMPACT 3RD',
    bannerCategory: '主线全新探索 ❖',
    bannerTitle: '「百年孤影」与火而战',
    bannerSubtitle: '全新第一点五部版本现已震撼上映',
    bannerButtonText: '进入月球基地',
    bgImage: '/src/assets/images/star_rail_blade_bg_1780939473544.png', // Fallback beautiful bg
    pvCardImage: '/src/assets/images/star_rail_pv_card_1780939496529.png', // Fallback thumb
    pvTitle: '「寻梦起航」周年纪念PV',
    accentColor: 'indigo',
    sizeGB: 41.5,
    installSpeedMB: 65.4,
    news: [
      {
        title: '「为世界上所有的美好而战」周年回馈开启',
        date: '06/07',
        category: '活动',
      },
      {
        title: '第33.5章特别番外「异界之门」剧情更新！',
        date: '06/04',
        category: '活动',
      },
      {
        title: '崩坏3 全球服务器系统合并与维护细节说明',
        date: '06/01',
        category: '公告',
      },
      {
        title: '关于防范不正当网络插件牟利等行径的法律警告',
        date: '05/29',
        category: '公告',
      },
      {
        title: '女武神「幽兰黛尔」精雕典藏手办实物预售开启',
        date: '05/27',
        category: '资讯',
      },
    ],
    socials: [
      { name: 'QQ', icon: 'QQ', tooltip: '加入官方QQ交流群', url: 'https://im.qq.com/' },
      { name: 'Discord', icon: 'Discord', tooltip: '加入官方 Discord 社区', url: 'https://discord.gg/honkaiimpact3rd' },
      { name: '官网', icon: 'Globe', tooltip: '访问崩坏3官方网站', url: 'https://honkaiimpact3.hoyoverse.com/' },
      { name: 'GitHub', icon: 'Github', tooltip: '查看 GitHub 开源仓库', url: 'https://github.com/hoyoverse' }
    ]
  }
];
