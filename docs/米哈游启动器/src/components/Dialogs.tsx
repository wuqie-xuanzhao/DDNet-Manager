import React, { useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { X, Volume2, HardDrive, Cpu, AlertTriangle, MessageSquare, Heart, Eye } from 'lucide-react';
import { GameNewsItem } from '../types';

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
                <span>来源: 米游社官方通讯</span>
                <span className="flex items-center">
                  <Eye className="w-3.5 h-3.5 mr-1" />
                  99,999+
                </span>
              </div>

              {/* Mock Rich Content Body */}
              <div className="space-y-4">
                <p>
                  亲爱的开拓者 / 旅人 / 代理人：
                </p>
                <p>
                  为了在全新版本更新后给各位提供更丰富、更多元、更纯粹的内容探索体验，我们官方社区现已正式开启专属征集活动与签到福利大升级行动！
                </p>
                
                <div className="bg-white/5 border-l-4 border-amber-400 p-3.5 rounded-lg text-gray-300/90 text-xs">
                  <strong>💡 核心提示：</strong> 活动期间，凡在米游社相关板块提交攻略、参与绘画绘制，或在游戏大地图标记探索心得的玩家，均有机会赢取高达 <strong>3000 原石/星琼</strong>、以及限定周边好礼！
                </div>

                <p className="font-semibold text-white mt-4">【参与方式 & 规则说明】</p>
                <ol className="list-decimal pl-5 space-y-2 text-xs">
                  <li>登录米游社客户端，在对应游戏板块找到对应置顶帖或活动入口；</li>
                  <li>按照活动模板规范，在指定话题标签下发布您的原创投稿；</li>
                  <li>严禁抄袭、恶意灌水或发布含有不良引流、广告的内容，违者永久禁赛；</li>
                  <li>评选结果将于活动截止后 15 个工作日内公示并发放游戏内奖励邮件。</li>
                </ol>

                <p>
                  跨越漫天星海与重重空洞，让我们在全新篇章里再度聚首。期待您的绝佳创意！
                </p>
              </div>

              {/* Like / Share panel */}
              <div className="flex items-center justify-between border-t border-white/5 pt-5 mt-6 text-xs text-gray-400">
                <div className="flex space-x-3">
                  <button className="flex items-center space-x-1 hover:text-red-400 transition-colors p-1">
                    <Heart className="w-4 h-4 text-red-400 fill-red-400" />
                    <span>23.5k 赞</span>
                  </button>
                  <button className="flex items-center space-x-1 hover:text-amber-400 transition-colors p-1">
                    <MessageSquare className="w-4 h-4" />
                    <span>1,842 评论</span>
                  </button>
                </div>
                <span>米哈游社区运行团队 敬上</span>
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


// --- 2. SETTINGS PANEL MODAL ---
interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  speedLimit: boolean;
  setSpeedLimit: (limit: boolean) => void;
  bgmVolume: number;
  setBgmVolume: (vol: number) => void;
  isAudioOn: boolean;
  setIsAudioOn: (on: boolean) => void;
  showGameIcons: boolean;
  setShowGameIcons: (show: boolean) => void;
}

type SettingsTab = '通用' | '下载' | '通知' | '工具' | '关于';

export function SettingsModal({
  isOpen,
  onClose,
  speedLimit,
  setSpeedLimit,
  bgmVolume,
  setBgmVolume,
  isAudioOn,
  setIsAudioOn,
  showGameIcons,
  setShowGameIcons,
}: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>('通用');
  const [autoLaunch, setAutoLaunch] = useState(false);
  const [autoPopup, setAutoPopup] = useState(true);
  const [closeAction, setCloseAction] = useState<'tray' | 'quit'>('quit');
  const [silentUpdate, setSilentUpdate] = useState(false);
  const [installPath, setInstallPath] = useState('D:\\Program Files\\mhy\\HoYoPlay');

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <div className="absolute inset-0 flex items-center justify-center z-50 select-none">
        {/* Backdrop overlay closely mimicking a modern client application blur */}
        <motion.div
          id="settings-modal-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
          className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        />

        {/* Spacious high fidelity Settings Box */}
        <motion.div
          id="settings-modal-content"
          initial={{ opacity: 0, scale: 0.95, y: 15 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, y: 15 }}
          transition={{ duration: 0.22, ease: 'easeOut' }}
          className="relative w-[680px] h-[480px] bg-[#1a1c22] border border-white/10 rounded-2xl shadow-[0_12px_40px_rgba(0,0,0,0.6)] overflow-hidden flex flex-col font-sans text-white/90"
        >
          {/* Main Content Area (Split into Sidebar and Config Panel) */}
          <div className="flex-1 flex overflow-hidden">
            
            {/* LEFT SIDEBAR VIEW SECTOR */}
            <div className="w-[180px] bg-[#131418] border-r border-white/5 p-4 flex flex-col justify-between">
              <div className="space-y-1.5">
                {/* Custom subtitle tag */}
                <span className="text-white text-base font-bold pl-2.5 pb-2.5 block tracking-wide">
                  设置
                </span>
                
                {/* Set of Tabs in HoYoPlay style list */}
                {(['通用', '下载', '通知', '工具', '关于'] as SettingsTab[]).map((tab) => {
                  const isActive = activeTab === tab;
                  return (
                    <button
                      key={tab}
                      id={`settings-tab-${tab}`}
                      onClick={() => setActiveTab(tab)}
                      className={`w-full text-left px-3.5 py-2 rounded-lg text-xs font-semibold tracking-wide transition-all ${
                        isActive
                          ? 'bg-white/10 text-white shadow-sm border border-white/5'
                          : 'text-gray-400 hover:text-gray-200 hover:bg-white/5'
                      } cursor-pointer`}
                    >
                      {tab}
                    </button>
                  );
                })}
              </div>

              {/* Version mark in left corner */}
              <div className="pl-2.5 pb-1 text-[10px] text-gray-500 font-mono">
                版本 v1.0.42_STABLE
              </div>
            </div>

            {/* RIGHT SCROLLABLE DETAILS PANEL */}
            <div className="flex-1 bg-[#1a1c22] p-6 overflow-y-auto">
              {activeTab === '通用' && (
                <div className="space-y-6">
                  
                  {/* Category 1: 启动设置 */}
                  <div className="space-y-3">
                    <div className="flex items-center justify-between border-b border-white/5 pb-1">
                      <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                        启动设置
                      </span>
                    </div>

                    {/* Startup line 1 */}
                    <div className="flex items-center justify-between py-1">
                      <div className="flex items-center space-x-1.5">
                        <span className="text-xs text-gray-200 font-medium">
                          开机自动运行米哈游启动器
                        </span>
                        <span className="bg-[#ffe082]/10 border border-[#ffe082]/20 text-yellow-500 text-[9px] font-bold px-1 py-[1px] rounded-sm scale-90">
                          推荐
                        </span>
                      </div>
                      
                      {/* Interactive toggle switch */}
                      <button
                        id="toggle-auto-launch"
                        onClick={() => setAutoLaunch(!autoLaunch)}
                        className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
                          autoLaunch ? 'bg-yellow-400' : 'bg-white/10'
                        }`}
                      >
                        <div
                          className={`w-[14px] h-[14px] rounded-full transition-transform bg-[#fff] shadow-sm ${
                            autoLaunch ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>

                    {/* Startup line 2 */}
                    <div className="flex items-center justify-between py-1">
                      <span className="text-xs text-gray-200 font-medium">
                        退出游戏后自动弹出启动器主界面
                      </span>
                      
                      {/* Active ON Toggle switch */}
                      <button
                        id="toggle-auto-popup"
                        onClick={() => setAutoPopup(!autoPopup)}
                        className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
                          autoPopup ? 'bg-yellow-400' : 'bg-white/10'
                        }`}
                      >
                        <div
                          className={`w-[14px] h-[14px] rounded-full transition-transform bg-[#fff] shadow-sm ${
                            autoPopup ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>
                  </div>

                  {/* Category 1.5: 边栏设置 */}
                  <div className="space-y-3">
                    <div className="flex items-center justify-between border-b border-white/5 pb-1">
                      <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                        边栏设置
                      </span>
                    </div>

                    <div className="flex items-center justify-between py-1">
                      <span className="text-xs text-gray-200 font-medium">
                        在左侧边栏显示游戏快捷图标
                      </span>
                      
                      <button
                        id="toggle-show-game-icons"
                        onClick={() => setShowGameIcons(!showGameIcons)}
                        className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
                          showGameIcons ? 'bg-yellow-400' : 'bg-white/10'
                        }`}
                      >
                        <div
                          className={`w-[14px] h-[14px] rounded-full transition-transform bg-[#fff] shadow-sm ${
                            showGameIcons ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>
                  </div>

                  {/* Category 2: 关闭设置 */}
                  <div className="space-y-3">
                    <div className="flex items-center justify-between border-b border-white/5 pb-1">
                      <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                        关闭设置
                      </span>
                    </div>

                    <div className="space-y-2">
                      <span className="text-xs text-gray-400 pl-0.5">关闭窗口：</span>

                      {/* Frame box enclosing Close Action radio items */}
                      <div className="bg-[#121316] border border-white/5 rounded-xl p-3 space-y-2.5">
                        
                        {/* Radio Choice 1: Minimize to System Tray */}
                        <button
                          id="radio-close-tray"
                          onClick={() => setCloseAction('tray')}
                          className="w-full flex items-center justify-between text-left cursor-pointer group"
                        >
                          <div className="flex items-center space-x-2.5">
                            {/* Custom radio button node */}
                            <div className={`w-3.5 h-3.5 rounded-full border flex items-center justify-center transition-all ${
                              closeAction === 'tray' ? 'border-yellow-400 ring-2 ring-yellow-400/25' : 'border-gray-500 group-hover:border-gray-400'
                            }`}>
                              {closeAction === 'tray' && <div className="w-2 h-2 rounded-full bg-yellow-400" />}
                            </div>
                            <span className="text-xs text-gray-300 font-medium group-hover:text-white-85">
                              最小化到系统托盘
                            </span>
                          </div>
                          <span className="bg-[#ffe082]/5 text-yellow-500 text-[8px] font-bold px-1.5 py-[1px] rounded">
                            推荐
                          </span>
                        </button>

                        <div className="h-[1px] bg-white/[0.03]" />

                        {/* Radio Choice 2: Exit launcher completely */}
                        <button
                          id="radio-close-quit"
                          onClick={() => setCloseAction('quit')}
                          className="w-full flex items-center space-x-2.5 text-left cursor-pointer group"
                        >
                          {/* Custom radio button node */}
                          <div className={`w-3.5 h-3.5 rounded-full border flex items-center justify-center transition-all ${
                            closeAction === 'quit' ? 'border-yellow-400 ring-2 ring-yellow-400/25' : 'border-gray-500 group-hover:border-gray-400'
                          }`}>
                            {closeAction === 'quit' && <div className="w-2 h-2 rounded-full bg-yellow-400" />}
                          </div>
                          <span className="text-xs text-gray-300 font-medium group-hover:text-white-85">
                            退出启动器
                          </span>
                        </button>

                      </div>
                    </div>
                  </div>

                  {/* Category 3: 启动器更新 */}
                  <div className="space-y-3">
                    <div className="flex items-center justify-between border-b border-white/5 pb-1">
                      <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                        启动器更新
                      </span>
                    </div>

                    {/* Updater Line 1 */}
                    <div className="flex items-center justify-between py-1">
                      <div className="flex items-center space-x-1.5">
                        <span className="text-xs text-gray-200 font-medium">
                          允许静默更新
                        </span>
                        <span className="bg-gray-700/50 text-gray-400 text-[9px] font-medium px-1 py-[1px] rounded-sm scale-90">
                          省心
                        </span>
                      </div>
                      
                      {/* Quiet update toggle button */}
                      <button
                        id="toggle-silent-update"
                        onClick={() => setSilentUpdate(!silentUpdate)}
                        className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
                          silentUpdate ? 'bg-yellow-400' : 'bg-white/10'
                        }`}
                      >
                        <div
                          className={`w-[14px] h-[14px] rounded-full transition-transform bg-[#fff] shadow-sm ${
                            silentUpdate ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>
                  </div>

                </div>
              )}

              {activeTab === '下载' && (
                <div className="space-y-5">
                  <div className="flex items-center justify-between border-b border-white/5 pb-1">
                    <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                      下载安装选项
                    </span>
                  </div>

                  {/* File directory picker */}
                  <div className="space-y-2.5">
                    <label className="text-xs text-gray-400 font-medium block">
                      <HardDrive className="w-3.5 h-3.5 inline mr-1 text-yellow-400" />
                      默认安装路径
                    </label>
                    <div className="flex space-x-2">
                      <input
                        type="text"
                        value={installPath}
                        onChange={(e) => setInstallPath(e.target.value)}
                        className="bg-black/30 border border-white/10 rounded-lg px-3.5 py-2 w-full text-xs text-gray-300 focus:outline-none focus:border-yellow-400/40 font-mono"
                      />
                      <button
                        onClick={() => alert('请在模拟文件浏览器中选择 (此弹窗已拦截)')}
                        className="bg-white/5 hover:bg-white/10 border border-white/5 px-3.5 rounded-lg text-xs font-semibold text-white shrink-0 transition-colors cursor-pointer"
                      >
                        浏览...
                      </button>
                    </div>
                  </div>

                  {/* Speed limiter */}
                  <div className="flex items-center justify-between py-3 border-y border-white/5">
                    <div>
                      <span className="text-white text-xs font-semibold block">
                        限制下载网速
                      </span>
                      <span className="text-[10px] text-gray-500 block leading-normal mt-0.5 max-w-sm">
                        开启后将下载上限限制在 12.0 MB/s 以防挤占多端在线宽带
                      </span>
                    </div>
                    <button
                      id="toggle-speed-limit"
                      onClick={() => setSpeedLimit(!speedLimit)}
                      className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
                        speedLimit ? 'bg-yellow-400' : 'bg-white/10'
                      }`}
                    >
                      <div
                        className={`w-[14px] h-[14px] rounded-full transition-transform bg-[#fff] shadow-sm ${
                          speedLimit ? 'translate-x-5' : 'translate-x-0'
                        }`}
                      />
                    </button>
                  </div>
                </div>
              )}

              {activeTab === '通知' && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between border-b border-white/5 pb-1">
                    <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                      通知与信使提醒
                    </span>
                  </div>

                  <div className="p-3 bg-white/5 border border-white-[0.03] rounded-xl space-y-2 text-xs text-gray-400 leading-relaxed">
                    <p className="font-semibold text-gray-200">消息广播通告</p>
                    <p>开启后，启动器将自动接收最新公测版本更新、限时福利大放送、以及女武神、降临角色跃迁等重要活动通知短信。</p>
                  </div>

                  <div className="flex items-center justify-between py-1">
                    <span className="text-xs text-gray-200 font-medium">允许启动器发送系统托盘横幅提醒</span>
                    <div className="w-10 h-5 rounded-full bg-yellow-400 flex items-center px-[3px]">
                      <div className="w-[14px] h-[14px] rounded-full bg-[#fff] translate-x-5" />
                    </div>
                  </div>
                </div>
              )}

              {activeTab === '工具' && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between border-b border-white/5 pb-1">
                    <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                      环境与音量调节
                    </span>
                  </div>

                  {/* BGM Controls */}
                  <div className="space-y-3 p-4 bg-white/5 rounded-xl border border-white-[0.03]">
                    <div className="flex items-center justify-between">
                      <div>
                        <span className="text-white text-xs font-semibold block">
                          <Volume2 className="w-3.5 h-3.5 inline mr-1 text-yellow-400" />
                          自适应背景环境音 (BGM)
                        </span>
                        <span className="text-[10px] text-gray-500 block leading-normal mt-0.5">
                          切换不同自研游戏时，是否启动专属背景音乐氛围声效
                        </span>
                      </div>

                      <button
                        id="toggle-settings-audio"
                        onClick={() => setIsAudioOn(!isAudioOn)}
                        className={`w-10 h-5 rounded-full flex items-center transition-colors px-[3px] cursor-pointer ${
                          isAudioOn ? 'bg-yellow-400' : 'bg-white/10'
                        }`}
                      >
                        <div
                          className={`w-[14px] h-[14px] rounded-full transition-transform bg-[#fff] shadow-sm ${
                            isAudioOn ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>

                    <div className={`flex items-center space-x-3 pt-1 transition-opacity duration-200 ${isAudioOn ? 'opacity-100' : 'opacity-40 pointer-events-none'}`}>
                      <span className="text-xs text-gray-400 w-12 font-mono">【{Math.round(bgmVolume * 100)}%】</span>
                      <input
                        type="range"
                        min="0"
                        max="1"
                        step="0.05"
                        value={bgmVolume}
                        onChange={(e) => setBgmVolume(parseFloat(e.target.value))}
                        className="w-full accent-yellow-400 bg-white/10 rounded-lg appearance-none h-1 cursor-pointer"
                      />
                    </div>
                  </div>
                </div>
              )}

              {activeTab === '关于' && (
                <div className="space-y-4 text-xs text-gray-400 leading-relaxed">
                  <div className="flex items-center justify-between border-b border-white/5 pb-1">
                    <span className="text-gray-400 text-xs font-bold uppercase tracking-wider">
                      关于启动器
                    </span>
                  </div>

                  <div className="flex items-center space-x-3 p-1">
                    <div className="w-11 h-11 rounded-xl bg-gradient-to-tr from-yellow-400 to-orange-500 flex items-center justify-center font-black tracking-widest text-lg text-white font-mono shadow-md shadow-yellow-500/10">
                      H
                    </div>
                    <div>
                      <span className="text-white font-medium block">米哈游统一启动面板（HoYoPlay）</span>
                      <p className="text-[10px] text-gray-500 font-mono mt-0.5">Build version: 1.0.42_STABLE_AI_BUILD</p>
                    </div>
                  </div>

                  <p className="pt-2">米哈游自研的精品游戏集中平台。旨在提供极速分发体验、全线背景皮肤智能联动、多端跨网络极低损耗。让女武神、列车组、旅行者与绝区绳匠共聚一堂，开启奇妙旅程。</p>
                  
                  <div className="h-[1px] bg-white/5 my-3" />
                  
                  <p className="text-[10px] text-gray-600 block text-center">Copyright © miHoYo. All Rights Reserved.</p>
                </div>
              )}
            </div>

          </div>

          {/* Bottom Footer Action buttons */}
          <div className="bg-[#131418] p-4.5 border-t border-white/5 flex justify-end space-x-3.5 shrink-0">
            <button
              onClick={onClose}
              className="px-5 py-2 rounded-lg bg-white/5 hover:bg-white/10 text-gray-300 text-xs font-semibold cursor-pointer transition-colors"
            >
              取消
            </button>
            <button
              id="btn-settings-save"
              onClick={() => {
                alert('设置已成功保存！');
                onClose();
              }}
              className="px-6 py-2 rounded-lg bg-yellow-400 hover:bg-yellow-300 text-black text-xs font-bold cursor-pointer transition-all"
            >
              确 认
            </button>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}


// --- 3. CONFIRM EXIT LAUNCHER DIALOG ---
interface ConfirmExitModalProps {
  isOpen: boolean;
  onCancel: () => void;
  onConfirmExit: () => void;
}

export function ConfirmExitModal({ isOpen, onCancel, onConfirmExit }: ConfirmExitModalProps) {
  const [option, setOption] = useState<'tray' | 'quit'>('tray');
  const [dontRemind, setDontRemind] = useState(false);

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <div className="absolute inset-0 flex items-center justify-center z-50">
        <motion.div
          id="exit-modal-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onCancel}
          className="absolute inset-0 bg-black/80 backdrop-blur-md"
        />

        <motion.div
          id="exit-modal-content"
          initial={{ opacity: 0, scale: 0.9, y: 30 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.9, y: 30 }}
          className="w-[440px] bg-[#161821] border border-white/10 rounded-2xl shadow-2xl p-6 flex flex-col space-y-5"
        >
          {/* Header Warning */}
          <div className="flex items-center space-x-2.5">
            <AlertTriangle className="w-5 h-5 text-amber-400" />
            <span className="text-white text-base font-bold tracking-wide">
              关闭启动器
            </span>
          </div>

          <p className="text-sm text-gray-300">
            你点击了关闭面板。请选择接下来米哈游启动器（HoYoPlay）的处理模式：
          </p>

          {/* Mode Selector Radio Options */}
          <div className="flex flex-col space-y-2.5">
            <button
              id="exit-opt-tray"
              onClick={() => setOption('tray')}
              className={`p-3 rounded-xl border text-left flex items-center space-x-3.5 transition-all text-xs cursor-pointer ${
                option === 'tray'
                  ? 'bg-amber-400/10 border-amber-400/40 text-white'
                  : 'bg-white/5 border-transparent text-gray-400 hover:bg-white/10'
              }`}
            >
              <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0 ${
                option === 'tray' ? 'border-amber-400' : 'border-gray-500'
              }`}>
                {option === 'tray' && <div className="w-2 h-2 rounded-full bg-amber-400" />}
              </div>
              <div>
                <span className="font-semibold block">最小化到系统托盘 (推荐)</span>
                <span className="text-[10px] text-gray-500 mt-0.5">隐藏主窗口，使安装更新与下载能在后台安静完成</span>
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
                <span className="font-semibold block">干脆退出启动器</span>
                <span className="text-[10px] text-gray-500 mt-0.5">关闭并停止所有正在进行的加速线程与后台下载任务</span>
              </div>
            </button>
          </div>

          {/* Under options dont remind banner */}
          <label className="flex items-center space-x-2.5 text-xs text-gray-500 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={dontRemind}
              onChange={(e) => setDontRemind(e.target.checked)}
              className="rounded border-white/10 bg-white/5 accent-amber-400"
            />
            <span>下次关闭时记住此选择，不再提示此弹窗</span>
          </label>

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
                if (option === 'tray') {
                  alert('模拟：启动器已最小化至托盘运行！');
                  onCancel();
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
