# DDNet Manager 米哈游启动器重设计 PRD

**日期**: 2026-06-18
**作者**: Proma Agent + 鸢天
**状态**: 已确认，进入实施

---

## 1. 现状分析

**主界面已经是米哈游结构**——`App.tsx` + `data.ts` 里 `GAMES_DATA` 硬编码 4 个 game tab（qmclient / ddnet / ddnet-steam / third-party），每个 tab 有独立背景图、accent 色、新闻卡片、社交链接。顶部 tab 切换 + 中间大 Banner + 右侧"获取/开始游戏"按钮 + 底部新闻。**视觉外壳已经对齐米哈游**。

**但数据层完全脱节**：
1. **4 个 tab 是写死的**，扫描到的客户端要硬塞进 `clientMatchesGameId` 的 4 个桶（`useClientLauncher.ts:88-108`），BestClient/TaterClient/CactusClient 都被丢进"第三方"
2. **DownloadButton 是 mock**——`useState(canLaunch ? 'installed' : 'uninstalled')` + `setInterval` 假装下载进度（`DownloadButton.tsx:55-113`），点了"获取游戏"啥也没下，纯演示
3. **真下载逻辑在 UpdatePanel**（设置 → 下载页），通过 `start_update_download` + `install_downloaded_update` IPC 跑完整 check → download → verify → install 流程，但**完全和主界面隔离**
4. **扫描功能完全藏在设置里**——`scan_clients_via_mft` 是 Tauri command 已能多盘并行扫，但入口是 `设置 → 客户端 → 扫描常见路径`，**主界面根本不会自动扫**
5. **`useClientLauncher.refreshLaunchReadiness`** 只调 `list_client_installations()` 拉已保存的，registry 空 → "游戏未录入，请点击获取或定位游戏"

**结论**：现状是"漂亮的米哈游壳 + 设备管理器内核"。要做的是把数据流接通——**让扫描、版本检查、下载、安装都在主界面闭环**，而不是跳到设置页。

---

## 2. 核心机制

**主界面状态机**（每个 tab 独立）：

```
[启动器冷启动] → 只查 registry（瞬时），不主动扫描
  registry 有记录                  → 直接显示"开始游戏"或"更新游戏"（见下）
  registry 无记录                  → 显示"获取游戏"（主按钮）+ "定位游戏"（小链接）

[点"定位游戏"或"获取游戏"才触发后台扫描]
  priority roots 秒扫 → 命中        → 自动落 registry，切到"开始游戏"
  priority 未命中 → 全盘 fallback   → 命中同上；未命中提示用户可点"获取游戏"真下载

[版本检查] 启动后后台异步拉 6 个客户端的 GitHub Release，事件驱动刷新 tab 状态
```

**关键按钮交互（视觉零变化）**——复用现有"已安装？定位游戏"那个小链接模式：

| 状态 | 主按钮 | 下方小链接 |
|---|---|---|
| 未在 registry、未扫过 | "获取游戏" | "已安装？定位游戏"（点=触发后台扫描） |
| 已扫确认没装 | "获取游戏"（点=真下载最新版） | "已安装？定位游戏" |
| 已装最新版 | "开始游戏" | （不显示，或显示版本号 vX.Y.Z） |
| **已装旧版** | **"开始游戏"** | **"获取更新 vX.Y.Z →"**（点=触发更新下载） |
| 下载中 | 进度条 + 暂停 | "取消" |
| 校验中 | "校验中…" 旋转 | — |
| 已装但残缺 | "修复" | "重新定位" |

**核心改进**：
- 检测到更新**不换主按钮**（仍是"开始游戏"），只把下方小链接文案从 "已安装？定位游戏" 改成 "获取更新 vX.Y.Z →"。视觉布局完全不变，只是文字 + 动作变。模仿米哈游"开始游戏"下方小字"预下载"那个模式。
- 用户想更新就点小链接；不想更新就点主按钮直接玩。**零打扰，零视觉跳动**。
- "定位游戏" 既是入口也是"扫描触发器"——用户点一次后台扫一遍，扫到就落 registry。比启动时全员扫描省 IO，且符合用户心智（"我装了 → 点定位让它认"）。

**右上角启动器自更新按钮**（独立于客户端更新）：
- 现状：`useAutoUpdate` hook 已存在但 UI 入口埋在设置-关于，发现更新后引导用户去 GitHub 用浏览器下载
- 新：右上角加图标按钮（下载图标 + 红点 = 有更新），点击展开小卡片"发现新版本 vX.Y.Z" + 自动下载进度 + 下载完成"点击安装"
- 设置-关于保留"自动下载启动器更新"开关（默认开），关闭后右上角按钮静默
- 真正的"静默自动更新"：后台下载完，提示用户安装（不强制重启）

**窗口尺寸对齐**：当前 2240×1332 → 目标 2240×1260（与米哈游一致）。72px 差值多半在 banner 高度、底部新闻区高度、或 padding 上。需要量一遍找具体差距源。

---

## 3. "获取游戏"安装弹窗

**触发**：点主按钮"获取游戏"（任何状态：未装 / 已扫确认没装）→ 弹出模态。

**弹窗结构**（参考米哈游/Steam 安装对话框）：

```
┌─ 安装 QmClient ─────────────────────────────┐
│                                              │
│  安装位置                                     │
│  ┌──────────────────────────────────────┐  │
│  │ 📁 C:\DDNet\Clients\QmClient\v1.2.3   │  │
│  │ SSD · 剩余 412 GB              [更改] │  │
│  └──────────────────────────────────────┘  │
│                                              │
│  快捷方式                                     │
│  ☑ 创建桌面快捷方式                          │
│  ☑ 创建开始菜单快捷方式                       │
│                                              │
│  [macOS 专属]                                │
│  管理模式                                     │
│  ◉ 在本启动器内独立管理（推荐）                │
│     装到 ~/Library/Application Support/      │
│     DDNetManager/clients/DDNet.app           │
│  ◉ 替换 /Applications/DDNet.app              │
│     （若已存在则备份后替换）                   │
│                                              │
│  版本信息                                     │
│  v1.2.3 (最新)  ·  920 MB  ·  GitHub Release │
│                                              │
├──────────────────────────────────────────────┤
│  已安装？定位游戏            [开始安装]      │
└──────────────────────────────────────────────┘
```

**关键技术点**：

1. **SSD/HDD 检测 + 剩余空间**：
   - Windows：调 `GetDiskFreeSpaceExW` + `DeviceIoControl(IOCTL_STORAGE_QUERY_PROPERTY)` 查 rotational
   - Linux：读 `/sys/block/<dev>/queue/rotational` + `statvfs`
   - macOS：用 `statfs` + diskutil 信息
   - 默认推荐路径选**当前已选盘**上的启动器自管理目录（如 Windows 的 `%LOCALAPPDATA%\DDNetManager\clients\<client_id>\v<version>\`），用户可改

2. **快捷方式创建**（安装完成后调一次）：
   - Windows：`IShellLinkW` + `IPersistFile` 创建 .lnk 到桌面 / `%APPDATA%\Microsoft\Windows\Start Menu\Programs\`
   - Linux：写 `.desktop` 文件到 `~/.local/share/applications/` 和 `~/Desktop/`
   - macOS：跳过（dock 已是事实标准；选"在本启动器内独立管理"时 dock 入口由启动器自己处理）

3. **macOS 双模式**（弹窗顶部 Radio 切换）：
   - **独立管理**（推荐）：装到启动器的 `~/Library/Application Support/DDNetManager/clients/DDNet.app`，不动 `/Applications/DDNet.app`。优势：用户原有的 Steam/官网装的不被破坏，启动器能精确控制版本
   - **替换 /Applications**：检测现有 `.app` → 备份到 `<original>.ddnet-manager-backup-<timestamp>` → 替换。用户主动选这个才执行，避免误操作

4. **默认安装位置算法**：
   - 第一启动：默认 = 启动器 cache 目录下的 `clients/<client_id>/v<version>/`
   - 之后记住上次选择（持久化到 settings，按 client_id 记忆）

**版本信息必须真实，禁止占位**：
- **版本号 vX.Y.Z**：来自 `ClientUpdateCheck.latest_version`（启动时已预拉，弹窗打开秒读）
- **下载体积**：来自 `ClientUpdateCheck.asset.size`（GitHub API `assets[].size` 字节数）
- **GitHub Release 链接**：来自 `ClientUpdateCheck.release_url`，可点击 → 用 `shell.open()` 跳转外部浏览器
- 拉取失败时弹窗显示错误状态卡片（"无法获取版本信息"+ "重试"按钮），而不是 `vX.Y.Z` `XXX MB` 这种占位文本
- 启动时预拉的结果缓存到内存，弹窗秒开；缓存超过 5 分钟或失败时弹窗打开瞬间触发重拉（1-2s 骨架屏）

**安装流程**（点"开始安装"后）：弹窗关闭 → 主按钮变成下载进度条 → 真实下载 + sha256 校验 + 解压 + 创建快捷方式 → 完成 → 主按钮变"开始游戏"

---

## 4. 数据流与组件架构

**新增 hook：`useClientInstaller`**（统一下载/安装/状态机，DownloadButton 和 UpdatePanel 都消费）

```ts
// hooks/useClientInstaller.ts
type ClientInstallState =
  | { kind: "unknown" }                                      // 未在 registry、未扫过
  | { kind: "not_installed"; scanned: boolean }              // 扫描确认没装
  | { kind: "installed"; version: string; latest?: string } // 已装，latest 缺失=未拉到
  | { kind: "downloading"; progress: number; speed: number }
  | { kind: "verifying" }
  | { kind: "failed"; error: string }
  | { kind: "broken" };                                      // 已装但 health != Ok

useClientInstaller(gameId: string): {
  state: ClientInstallState;
  triggerScan(): Promise<void>;       // 点"定位游戏"触发后台扫描
  openInstallDialog(): void;          // 点"获取游戏"弹安装对话框
  openUpdateDialog(): void;           // 点"获取更新 vX.Y.Z"弹更新对话框（同结构，预填当前路径）
  launchGame(): Promise<void>;        // 点"开始游戏"
}
```

**数据来源**：
- `state` 由三股流合并（`useEffect` 订阅）：
  1. `list_client_installations()` —— registry 已保存的（瞬时）
  2. `scan_clients_via_mft` 事件流 —— 用户点"定位游戏"触发，命中后 upsert 到 registry，state 自动刷新
  3. `check_client_update` —— 启动后异步拉 6 个客户端 release 元数据，缓存到 React Context（避免每 tab 都拉）

**catalog/game tab 数据流改造**：
- `src/components/launcher/data.ts` 的 `GAMES_DATA` 硬编码 4 个 → 改为从 Rust 端 `client_catalog::catalog_entries()` 序列化到前端（动态 6 个）
- 每个 game tab 持有 catalog entry 引用：`{ client_id, display_name, upstream_url, update_source, accent_color, ... }`
- 视觉资产（背景图、PV 卡片）按 client_id 映射，新增客户端时补对应资源

**DownloadButton 重构**：
- 删除 mock `setInterval`（55-113 行）
- 状态从 `useClientInstaller.state` 推导，不再用 `useState`
- 监听 `download-progress` event 更新进度条（替换假 `setProgress`）

**UpdatePanel 不动**：保留作为"高级下载管理"页面（历史记录、断点续传、错误详情），主流程不在它走。`useClientInstaller` 和它共享底层 IPC（`start_update_download` / `install_downloaded_update`），不重复实现。

---

## 5. UI 治理与窗口尺寸

**shadcn 迁移范围**（项目已有 `card / button / badge / separator / collapsible`，需补全）：

| 新增 shadcn 组件 | 替换目标 |
|---|---|
| `dialog` | 安装弹窗、更新弹窗、退出确认 |
| `switch` | `SettingsDialog` 的 `Toggle` 自定义实现 |
| `radio-group` | `close_behavior` 三选一、macOS 管理模式双选 |
| `checkbox` | 创建桌面/开始菜单快捷方式 |
| `tabs` | 顶部 6 个 game tab |
| `progress` | `DownloadButton` 进度条（替换手写 div） |
| `tooltip` | tab hover 提示、小链接 hover 提示 |
| `alert-dialog` | "替换 /Applications/DDNet.app" 这种危险操作确认 |

**视觉风格统一**：
- `src/index.css` 已经有 `@theme inline` 把 `--color-primary` 映射到 `var(--app-accent)`，shadcn 组件会自动用 amber/yellow
- 需统一调：圆角（shadcn 默认 `rounded-md`，项目用 `rounded-xl`）、阴影、border 透明度
- 迁移完成后**全项目无手写 `<div className="bg-[#1f2229] border border-white/5 rounded-xl">` 这种卡片**，全部 `<Card>`

**窗口尺寸调整**（4K 1.5× 缩放下 2240×1332 → 2240×1260）：
- 逻辑高度 888 → 840（减 48px），改 `src-tauri/tauri.conf.json` 的窗口 height
- 量一遍找 72px 物理差距源（多半在 banner 高度 / 底部新闻卡 / 上下 padding），调整对应组件
- 配套：`min-h-[640px]` body 约束同步改

**迁移时机：A - 先迁移再做新功能**（地基稳）

---

## 6. 实施路线图

**阶段 0：UI 治理 / shadcn 迁移**（地基，~2-3 天）
- `bunx shadcn@latest add dialog switch radio-group checkbox tabs progress tooltip alert-dialog`
- `components.json` 配置，让 shadcn 用项目 amber/yellow（`@theme inline` 已铺好）
- 逐个迁移：SettingsDialog 的 Toggle/radio/卡片/推荐徽章、DownloadButton 的进度条、顶部 tab、各种弹窗
- 窗口尺寸 2240×1332 → 2240×1260（改 `tauri.conf.json` height + 量 72px 差距源）
- 验收：现有功能 100% 不变，但代码全是 shadcn 组件

**阶段 1：数据流通**（米哈游地基，~3-4 天）
- Rust catalog 序列化到前端：新增 `get_client_catalog` Tauri command
- `GAMES_DATA` 改为运行时从 catalog 拉，6 个动态 tab
- 新增 `useClientInstaller` hook（统一下载/安装/状态机）
- DownloadButton 删 mock setInterval，接入 hook
- 启动后后台并发拉 6 个客户端 release（事件驱动刷新 tab 状态）
- 点"定位游戏"触发 priority + 全盘扫描，命中后自动落 registry

**阶段 2：核心交互闭环**（~3-4 天）
- 安装弹窗（Dialog + 表单）
  - 路径选择 + SSD/HDD 检测 + 剩余空间（新增 Rust command `probe_disk`）
  - 快捷方式 checkbox
  - macOS 双模式 RadioGroup（独立管理 vs 替换 /Applications）
  - 版本信息：版本号、下载体积、GitHub Release 链接（必须真实，禁止占位）
- "获取更新 vX.Y.Z" 小链接 → 复用弹窗，预填当前路径
- 新增 Rust command `create_shortcuts`（Windows IShellLink / Linux .desktop / macOS skip）
- "开始安装"接 `start_update_download` + `install_downloaded_update` + 完成后 `create_shortcuts`

**阶段 3：启动器自更新**（~2 天）
- 右上角图标按钮（Download icon + 红点 = 有更新）
- 点击展开 Popover："发现 vX.Y.Z" + 自动下载进度 + "点击安装"
- 设置-关于加"自动下载启动器更新"开关（默认开）
- 自更新下载 + 安装流程（与客户端安装复用 useClientInstaller 的下载/校验/解压，但目标是启动器自身）
- 平台差异：Windows nsis / macOS dmg / Linux AppImage

**阶段 4：打磨与回归**（~2 天）
- 多副本客户端处理（用户装 2 个 QmClient → tab 内下拉切换）
- 残缺客户端修复流程（health != Ok 显示"修复"按钮）
- 全量 vitest 覆盖（useClientInstaller 状态机、弹窗交互、shadcn 组件迁移回归）
- 4K 缩放下窗口最终对齐验证
- Tauri dev 手动验收清单（按状态机每条路径走一遍）

**总工期：~12-15 个工作日**

---

## 7. 风险与取舍

- macOS 替换 /Applications/DDNet.app 需要管理员权限 → 系统弹确认，弹窗里要预先警告
- SSD/HDD 检测在某些 NAS / 网络盘上可能失败 → fallback 不显示标签
- GitHub API rate limit（未认证 60/h）→ 缓存 + 失败优雅降级，缓存键 = client_id + 5min TTL
- 启动器自更新要替换运行中的进程 → 下载完后提示用户重启，不强制
- shadcn 默认视觉（圆角 md、中性灰）与项目风格（圆角 xl、amber）差异 → 阶段 0 必须把 CSS 变量映射调好，否则后续阶段所有新组件都视觉错位

## 8. 不在范围内（明确放弃）

- 客户端 mod 管理 / 皮肤市场
- 多账号切换
- 云存档
- 启动器主题编辑器
- 远程启动 / 手机控制
