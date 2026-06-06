import { motion, useAnimationControls } from "framer-motion";
import {
  AlertCircle,
  CheckCircle2,
  LoaderCircle,
  Play,
  Rocket,
  Sparkles
} from "lucide-react";
import type { ClientInstallation, LauncherState } from "../../types";

type LaunchButtonProps = {
  state: LauncherState;
  disabled: boolean;
  onClick: () => Promise<void>;
};

type LaunchPanelProps = {
  state: LauncherState;
  clientPath: string;
  selectedClient: ClientInstallation | null;
  statusText: string;
  errorMessage: string | null;
  onClientPathChange: (value: string) => void;
  onBrowse: () => Promise<void>;
  onValidate: () => Promise<void>;
  onPrimaryAction: () => Promise<void>;
};

function LaunchButton(props: LaunchButtonProps) {
  const controls = useAnimationControls();

  const click = async () => {
    if (props.disabled) {
      return;
    }

    await controls.start({
      y: [0, -1, 0],
      scale: [1, 0.985, 1.01, 1],
      rotate: [0, -0.5, 0.5, 0],
      transition: { duration: 0.26, ease: "easeOut" }
    });

    await props.onClick();
  };

  const copy =
    props.state === "validating"
      ? {
          title: "验证中",
          subtitle: "正在检查目录结构和可执行文件",
          icon: <LoaderCircle className="animate-spin" size={22} />
        }
      : props.state === "ready"
        ? {
            title: "启动游戏",
            subtitle: "配置已通过验证，可以拉起客户端",
            icon: <Rocket size={22} />
          }
        : props.state === "launching"
          ? {
              title: "启动中",
              subtitle: "正在调用客户端进程，请稍候",
              icon: <LoaderCircle className="animate-spin" size={22} />
            }
          : props.state === "running"
            ? {
                title: "再次启动",
                subtitle: "上次启动请求已发送，可再次拉起客户端",
                icon: <CheckCircle2 size={22} />
              }
            : props.state === "error"
              ? {
                  title: "重新配置",
                  subtitle: "当前配置不可用，修正路径后再试",
                  icon: <AlertCircle size={22} />
                }
              : {
                  title: "选择客户端",
                  subtitle: "先指定 DDNet 或 QmClient 安装目录",
                  icon: <Play size={22} fill="currentColor" />
                };
  const badgeText =
    props.state === "ready"
      ? "Ready"
      : props.state === "running"
        ? "Launched"
        : props.state === "error"
          ? "Retry"
          : "Setup";

  return (
    <motion.button
      type="button"
      onClick={click}
      animate={controls}
      disabled={props.disabled}
      whileHover={props.disabled ? undefined : { y: -3, scale: 1.01 }}
      whileTap={props.disabled ? undefined : { scale: 0.99 }}
      className="group relative flex h-[86px] w-full items-center justify-between overflow-hidden rounded-[28px] border border-white/70 bg-[linear-gradient(135deg,#ff7fbc_0%,#ff9fc1_24%,#ffd781_62%,#8ddcff_100%)] px-7 text-left text-[#6f2f52] shadow-[0_22px_55px_rgba(255,137,189,0.30)] transition disabled:cursor-not-allowed disabled:opacity-70"
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(255,255,255,0.85),transparent_34%),radial-gradient(circle_at_bottom_right,rgba(255,255,255,0.35),transparent_28%)]" />
      <div className="absolute -right-6 -top-8 h-24 w-24 rounded-full bg-white/30 blur-xl transition group-hover:scale-110" />

      <div className="relative z-10 flex items-center gap-4">
        <div className="grid h-12 w-12 place-items-center rounded-full bg-white/78 text-[#ff5ea8] shadow-[inset_0_1px_0_rgba(255,255,255,0.95),0_10px_22px_rgba(255,94,168,0.20)]">
          {copy.icon}
        </div>
        <div>
          <div className="text-[28px] font-black tracking-[-0.04em] text-[#7b3660]">{copy.title}</div>
          <div className="text-xs font-bold tracking-[0.12em] text-[#8d5473]/80">{copy.subtitle}</div>
        </div>
      </div>

      <div className="relative z-10 flex items-center gap-2 rounded-full bg-white/55 px-4 py-2 text-[11px] font-black uppercase tracking-[0.16em] text-[#8f5870]">
        <Sparkles size={14} />
        {badgeText}
      </div>
    </motion.button>
  );
}

function HealthBadge(props: { state: LauncherState; text: string }) {
  const tone =
    props.state === "ready" || props.state === "running"
      ? "bg-[#dcfff2] text-[#2b8d63]"
      : props.state === "error"
        ? "bg-[#ffe0ea] text-[#d04c7f]"
        : props.state === "validating" || props.state === "launching"
          ? "bg-[#fff2c8] text-[#aa7a16]"
          : "bg-[#edf5ff] text-[#5e7fb4]";

  return (
    <div className={`inline-flex items-center gap-2 rounded-full px-4 py-2 text-xs font-black tracking-[0.14em] ${tone}`}>
      <span className="h-2 w-2 rounded-full bg-current opacity-70" />
      {props.text}
    </div>
  );
}

function MetaCard(props: { label: string; value: string; accent: string }) {
  return (
    <div className="rounded-[24px] border border-white/75 bg-white/68 p-4 shadow-[0_18px_40px_rgba(138,180,255,0.12)] backdrop-blur-xl">
      <div className="text-[11px] font-black uppercase tracking-[0.18em] text-[#8f9ab8]">{props.label}</div>
      <div className="mt-2 text-sm font-bold text-[#51627b]">
        <span className={`${props.accent} mr-2 inline-block h-2.5 w-2.5 rounded-full`} />
        {props.value}
      </div>
    </div>
  );
}

export function LaunchPanel(props: LaunchPanelProps) {
  const isReady = props.state === "ready";
  const canValidate = props.clientPath.trim().length > 0 && props.state !== "validating" && props.state !== "launching";
  const primaryDisabled = props.state === "validating" || props.state === "launching";

  return (
    <section className="grid gap-6 xl:grid-cols-[minmax(0,1.08fr)_320px]">
      <div className="rounded-[36px] border border-white/80 bg-white/66 p-7 shadow-[0_28px_80px_rgba(255,173,204,0.20)] backdrop-blur-2xl">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full bg-[#fff5fb] px-4 py-2 text-[11px] font-black uppercase tracking-[0.2em] text-[#e170aa]">
              <Sparkles size={14} />
              Candy Launch Deck
            </div>
            <h2 className="mt-4 text-[44px] font-black tracking-[-0.06em] text-[#5f5b86]">把你的客户端放进发射台</h2>
            <p className="mt-3 max-w-[600px] text-sm font-medium leading-7 text-[#73819c]">
              不再伪装更新、不再模拟下载。这里直接选择本地 DDNet 或 QmClient 安装目录，验证通过后再真实启动可执行文件。
            </p>
          </div>

          <HealthBadge state={props.state} text={props.statusText} />
        </div>

        <div className="mt-7 rounded-[28px] border border-white/75 bg-[linear-gradient(180deg,rgba(255,255,255,0.82),rgba(250,252,255,0.68))] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.95)]">
          <label htmlFor="client-path" className="text-[11px] font-black uppercase tracking-[0.22em] text-[#94a0bd]">
            客户端目录
          </label>
          <div className="mt-3 flex flex-col gap-3 xl:flex-row">
            <input
              id="client-path"
              value={props.clientPath}
              onChange={(event) => props.onClientPathChange(event.target.value)}
              placeholder="例如 D:/Games/QmClient"
              className="h-14 flex-1 rounded-[20px] border border-[#dbe7ff] bg-white/90 px-5 text-sm font-semibold text-[#60708c] outline-none transition placeholder:text-[#b2bfd8] focus:border-[#8bd8ff] focus:ring-4 focus:ring-[#bcecff]/60"
            />
            <button
              type="button"
              onClick={() => void props.onBrowse()}
              className="h-14 rounded-[20px] border border-white/75 bg-[#dff7ff] px-5 text-sm font-black text-[#4995bc] shadow-[0_14px_30px_rgba(141,220,255,0.22)] transition hover:-translate-y-0.5"
            >
              浏览目录
            </button>
            <button
              type="button"
              onClick={() => void props.onValidate()}
              disabled={!canValidate}
              className="h-14 rounded-[20px] border border-white/75 bg-[#fff0a9] px-5 text-sm font-black text-[#9b7b17] shadow-[0_14px_30px_rgba(255,215,129,0.24)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-60"
            >
              验证路径
            </button>
          </div>

          {props.errorMessage ? (
            <div className="mt-4 rounded-[20px] border border-[#ffc5d9] bg-[#fff1f6] px-4 py-3 text-sm font-semibold text-[#c25583]">
              {props.errorMessage}
            </div>
          ) : null}
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-2">
          <MetaCard
            label="安装目录"
            value={props.selectedClient?.install_dir ?? "尚未验证"}
            accent="bg-[#94dcff]"
          />
          <MetaCard
            label="可执行文件"
            value={props.selectedClient?.executable_path ?? "等待目录验证"}
            accent="bg-[#ff9ac2]"
          />
          <MetaCard
            label="数据目录"
            value={props.selectedClient?.data_dir ?? "等待目录验证"}
            accent="bg-[#ffe08d]"
          />
          <MetaCard
            label="当前识别"
            value={props.selectedClient ? `${props.selectedClient.display_name} · ${props.statusText}` : "未识别客户端"}
            accent="bg-[#b1efc3]"
          />
        </div>

        <div className="mt-6">
          <LaunchButton
            state={props.state}
            disabled={primaryDisabled}
            onClick={props.onPrimaryAction}
          />
          {!isReady && props.state !== "launching" && props.state !== "running" ? (
            <div className="mt-3 text-xs font-bold tracking-[0.1em] text-[#92a0ba]">
              主按钮会根据状态自动切换到“选择客户端”或“重新配置”，不会伪装成已运行。
            </div>
          ) : null}
        </div>
      </div>

      <div className="space-y-4">
        <div className="rounded-[32px] border border-white/80 bg-white/62 p-5 shadow-[0_22px_58px_rgba(115,188,255,0.14)] backdrop-blur-2xl">
          <div className="text-[11px] font-black uppercase tracking-[0.22em] text-[#8da3c5]">启动规则</div>
          <div className="mt-4 space-y-3 text-sm font-semibold leading-6 text-[#6f7c96]">
            <div>1. 先选目录，再验证 DDNet.exe、storage.cfg 与 data。</div>
            <div>2. 只有目录健康时才允许真实启动。</div>
            <div>3. 启动失败会保留错误信息，不会伪装成“Running”。</div>
          </div>
        </div>

        <div className="rounded-[32px] border border-white/80 bg-[linear-gradient(180deg,rgba(230,251,255,0.94),rgba(255,244,214,0.88))] p-5 shadow-[0_22px_58px_rgba(255,204,122,0.16)]">
          <div className="text-[11px] font-black uppercase tracking-[0.22em] text-[#8d96a9]">状态提示</div>
          <div className="mt-4 space-y-3">
            <div className="rounded-[20px] bg-white/72 px-4 py-3 text-sm font-semibold text-[#67809e]">未配置：等待用户选择目录。</div>
            <div className="rounded-[20px] bg-white/72 px-4 py-3 text-sm font-semibold text-[#67809e]">验证中：短暂锁定交互，避免重复提交。</div>
            <div className="rounded-[20px] bg-white/72 px-4 py-3 text-sm font-semibold text-[#67809e]">已拉起：仅表示启动请求成功发出。</div>
          </div>
        </div>
      </div>
    </section>
  );
}
