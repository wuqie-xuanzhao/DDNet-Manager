# DDNet Manager 米哈游启动器重设计 - 验收清单

**日期**: 2026-06-19
**对应 PRD**: `docs/plans/2026-06-18-mihoyo-launcher-redesign.md`

按 PRD 阶段 0-4 完整状态机每条路径列手动验收步骤。每项标注预期行为。

---

## 阶段 0：UI 治理 / shadcn 迁移

### 0.1 字体颜色系统化
- [ ] 打开设置 → 通用：所有文字在 dark / light 主题下都清晰可读（无白字看不清）
- [ ] 切换主题（设置 → 外观）：所有颜色跟随主题切换，无残留 hard-coded 颜色

### 0.2 Switch / RadioGroup 迁移
- [ ] 设置 → 通用 → 启动设置：3 个开关视觉是米哈游风格（w-11 h-6 amber 边框 + 白底勾选圆点）
- [ ] 设置 → 通用 → 关闭设置：3 个 radio 圆点视觉一致（amber 实心圆点 + 外圈描边）

### 0.3 工具页重构
- [ ] 设置 → 工具：排除路径是列表 + "添加文件夹"按钮（不是 textarea）
- [ ] 点"添加文件夹" → 弹文件浏览器，选目录后列表多一行
- [ ] 点列表项的删除按钮（Trash 图标）→ 路径移除
- [ ] 最大候选数 / 扫描超时是 stepper（- 数字 +），有"恢复默认"链接

### 0.4 窗口尺寸
- [ ] 4K 屏 1.5×/1.75× 缩放下，启动器窗口物理尺寸接近米哈游启动器（2240×1260）

### 0.5 保存状态条
- [ ] 改任何设置 → 左下角 sidebar 出现"保存中…"→"已保存 ✓"（2 秒淡出）
- [ ] 设置保存失败 → "保存失败"红色常驻

---

## 阶段 1：数据流通（米哈游模型）

### 1.1 Gallery 抽屉（左下角"全部游戏"）
- [ ] 主界面默认显示 DDNet（不是 QmClient）
- [ ] 点左下角"全部游戏"按钮 → 抽屉打开
- [ ] 抽屉里 5 个客户端卡片：DDNet / QmClient / TaterClient / BestClient / Cactus（无"第三方客户端"）
- [ ] DDNet 卡片在第一位
- [ ] 点任一卡片 → 主界面切换 + 抽屉关闭

### 1.2 状态机
- [ ] 未安装 tab（如 TaterClient）：主按钮"获取游戏" + 下方"已安装？定位游戏"小链接
- [ ] 已安装最新 tab：主按钮"开始游戏" + 下方版本号 vX.Y.Z
- [ ] 已安装旧版 tab：主按钮"开始游戏" + 下方"获取更新 vLatest →"小链接
- [ ] 下载中：主按钮变进度条 + 暂停按钮
- [ ] 校验中：旋转图标 + "校验中"
- [ ] 失败：主按钮"重试" + 错误文案

### 1.3 点"定位游戏"扫描
- [ ] 点未安装 tab 的"已安装？定位游戏" → 后台扫描（priority roots + 全盘）
- [ ] 命中 → 自动落 registry，主按钮切到"开始游戏"
- [ ] 未命中 → 主按钮文案区域提示未找到

### 1.4 启动游戏
- [ ] 点"开始游戏" → DDNet.exe 启动，**启动器窗口 hide 到托盘**（不是关闭）
- [ ] 任务栏右下角托盘有 DDNet Manager 图标
- [ ] 点托盘图标 → 启动器恢复显示
- [ ] 关闭游戏（settings.exit_game_show_launcher=true）→ 启动器自动 unminimize + 抢焦点

---

## 阶段 2：安装弹窗

### 2.1 弹窗打开
- [ ] 点未安装 tab 的"获取游戏"主按钮 → **InstallDialog 立即弹出**（不阻塞扫描）
- [ ] 弹窗标题：`安装 {客户端名}` 或 `更新 {客户端名}`（更新模式）

### 2.2 版本信息（真实数据，禁止占位）
- [ ] 弹窗顶部"版本信息"卡：显示真实 `vX.Y.Z`（不是 vX.Y.Z 占位）
- [ ] 显示下载体积（如 `12.34 MB`）
- [ ] GitHub Release 链接可点击 → 跳转外部浏览器
- [ ] release 拉取中 → Loader2 旋转 + "正在获取版本信息…"
- [ ] 拉取失败 → 不显示占位文本（按钮 disabled）

### 2.3 安装位置
- [ ] 默认路径 `<LOCALAPPDATA>\DDNetManager\clients\<game_id>\v<version>\`
- [ ] 点"更改" → 文件浏览器选目录
- [ ] 下方显示 `SSD · 剩余 X GB / Y GB`（真实 probe_disk 数据）
- [ ] HDD 时显示"建议安装在 SSD"

### 2.4 快捷方式
- [ ] "创建桌面快捷方式" checkbox 默认勾选
- [ ] "创建开始菜单快捷方式" checkbox 默认勾选
- [ ] 点"开始安装" → 真实下载 → 校验 → 解压 → 创建 .lnk（Windows）/ .desktop（Linux）

### 2.5 弹窗底部
- [ ] 左下"已安装？定位游戏"小链接 → 触发扫描
- [ ] 右下"开始安装"主按钮：路径为空或版本未拉到时 disabled

### 2.6 更新模式
- [ ] 已装旧版 tab 点"获取更新 vX.Y.Z →" → 弹窗 mode=update
- [ ] 标题变 `更新 {客户端名}`
- [ ] 跳过快捷方式 checkbox（已有快捷方式）

---

## 阶段 3：启动器自更新

### 3.1 右上角按钮（WindowControls 内部，5 个并列）
- [ ] 顺序：[更新] [声音] [设置] [最小化] [关闭]
- [ ] 更新按钮视觉和其他 4 个一致（w-8 h-8 rounded-md）

### 3.2 状态视觉
- [ ] idle / up-to-date：Download 图标，灰色（和其他按钮一致）
- [ ] checking：Loader2 旋转
- [ ] has-update：Download + 红点 pulse，**绿色**（emerald）
- [ ] failed：AlertCircle 红色

### 3.3 点击行为
- [ ] 点 has-update 按钮 → 展开卡片：`发现新版本 vX.Y.Z` + `当前 vX.Y.Z` + release notes + `前往下载`按钮
- [ ] 点"前往下载" → 外部浏览器跳转 release_url
- [ ] 点 failed 按钮 → 展开卡片：错误信息 + "重试"按钮
- [ ] 点 idle/up-to-date 按钮 → 强制重新检查

### 3.4 自动检查
- [ ] 启动后延迟 1.5s 自动检查（设置 → 通用 → "自动检查启动器更新"打开时）
- [ ] 关闭开关 → 重启启动器后右上角按钮消失（静默）
- [ ] 5min 冷却：连续点击不会重复调 API（除非 force）

---

## 阶段 4：打磨

### 4.1 多副本切换
- [ ] 装两个同 client_id 客户端（如同 2 个 QmClient）→ installed 状态下，主按钮下方显示"副本 1/2 ▾"
- [ ] 点"副本 1/2 ▾" → 展开列表，每项显示路径 + 版本 + 默认标记
- [ ] 点列表项 → 切换 selectedClient，state 重新评估（installed/broken）

### 4.2 测试覆盖
- [ ] `bun run test` 全绿（81 个测试）
- [ ] `cargo test --package ddnet-manager` 全绿
- [ ] `cargo test --package ntfs-search` 全绿

---

## 已知遗留事项（不在本次范围）

- **macOS 双模式 RadioGroup**：弹窗里有 UI 但实际 macOS 替换 /Applications 逻辑未实装（仅 Windows/Linux 全功能）
- **tauri-plugin-updater 自更新**：当前是"前往下载"跳浏览器，未做启动器内自动下载安装包
- **顶部 game tab → shadcn Tabs 迁移**：高度定制的卡片轮播 + library 抽屉，shadcn Tabs 不兼容，保留原实现
- **DownloadButton 进度条 → shadcn Progress**：阶段 1 整体重写时再迁
