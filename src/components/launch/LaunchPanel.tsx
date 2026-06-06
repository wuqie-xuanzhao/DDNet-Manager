import { AnimatePresence, motion, useAnimationControls } from "framer-motion";
import { useState, type ReactNode } from "react";
import { GameIcon } from "@/components/icons/GameIcon";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { LauncherState } from "../../types";

type LaunchButtonProps = {
  state: LauncherState;
  disabled: boolean;
  onClick: () => Promise<void>;
};

type LaunchPanelProps = {
  state: LauncherState;
  clientPath: string;
  errorMessage: string | null;
  mobileUpdateStatus: ReactNode;
  backgroundMode: "default" | "custom";
  onClientPathChange: (value: string) => void;
  onBrowse: () => Promise<void>;
  onValidate: () => Promise<void>;
  onPrimaryAction: () => Promise<void>;
  onBackgroundImageSelect: (file: File) => Promise<void>;
  onClearBackgroundImage: () => void;
};

function LaunchButton(props: LaunchButtonProps) {
  const controls = useAnimationControls();

  const click = async () => {
    if (props.disabled) {
      return;
    }

    await controls.start({
      x: [0, -2, 2, 0],
      y: [0, -2, 0],
      scale: [1, 0.985, 1.012, 1],
      transition: { duration: 0.24, ease: "easeOut" }
    });

    await props.onClick();
  };

  const stateLabel = props.state === "validating" ? "启动，正在验证" : props.state === "launching" ? "启动中" : "启动";

  return (
    <motion.button
      type="button"
      onClick={click}
      animate={controls}
      disabled={props.disabled}
      aria-label={stateLabel}
      whileHover={props.disabled ? undefined : { y: -3, scale: 1.01 }}
      whileTap={props.disabled ? undefined : { scale: 0.99 }}
      className="group relative grid h-14 min-w-[168px] place-items-center overflow-hidden rounded-[22px] bg-[var(--dm-ink)] px-7 text-center text-white shadow-[0_14px_30px_rgba(47,52,64,0.16)] transition disabled:cursor-not-allowed disabled:opacity-70 sm:min-w-[196px]"
    >
      <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(255,255,255,0.08),rgba(255,255,255,0))]" />
      <div className="relative z-10 text-[22px] font-black tracking-[-0.04em]">开始游戏</div>
    </motion.button>
  );
}

export function LaunchPanel(props: LaunchPanelProps) {
  const [isPathPanelOpen, setIsPathPanelOpen] = useState(false);
  const canValidate = props.clientPath.trim().length > 0 && props.state !== "validating" && props.state !== "launching";
  const primaryDisabled = props.state === "validating" || props.state === "launching";

  return (
    <section className="mx-auto flex h-full w-full max-w-[1360px] items-end justify-end">
      <motion.div
        initial={{ opacity: 0, y: 24 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.08, duration: 0.42, ease: "easeOut" }}
        className="grid w-full shrink-0 gap-4 pb-2"
      >
        <Collapsible
          open={isPathPanelOpen}
          onOpenChange={setIsPathPanelOpen}
          className="min-w-0 text-[var(--dm-ink)]"
        >
          <div className="flex items-center justify-end gap-3">
            <LaunchButton state={props.state} disabled={primaryDisabled} onClick={props.onPrimaryAction} />
            <CollapsibleTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                aria-label="路径设置"
                className={`h-14 w-14 shrink-0 rounded-[22px] border transition ${
                  isPathPanelOpen
                    ? "border-[var(--dm-ink)]/18 bg-[var(--dm-ink)] text-white shadow-[0_14px_30px_rgba(47,52,64,0.16)]"
                    : "border-[var(--dm-border)] bg-white/64 text-[var(--dm-muted-ink)] hover:-translate-y-0.5 hover:bg-white hover:text-[var(--dm-ink)]"
                }`}
              >
                <GameIcon name="settings" className="size-6" />
              </Button>
            </CollapsibleTrigger>
          </div>

          <AnimatePresence>
            {isPathPanelOpen ? (
              <CollapsibleContent forceMount asChild>
                <motion.div
                  initial={{ opacity: 0, y: -8, height: 0 }}
                  animate={{ opacity: 1, y: 0, height: "auto" }}
                  exit={{ opacity: 0, y: -8, height: 0 }}
                  transition={{ duration: 0.2, ease: "easeOut" }}
                  className="overflow-hidden"
                >
                  <div className="mt-4 rounded-[24px] border border-[var(--dm-border)] bg-white/76 p-3 shadow-[0_16px_44px_rgba(47,52,64,0.08)] backdrop-blur-2xl">
                    <label htmlFor="client-path" className="text-[10px] font-black uppercase tracking-[0.22em] text-[var(--dm-muted-ink)]">
                      路径
                    </label>
                    <div className="mt-2 grid gap-2 lg:grid-cols-[minmax(0,1fr)_auto_auto]">
                      <input
                        id="client-path"
                        value={props.clientPath}
                        onChange={(event) => props.onClientPathChange(event.target.value)}
                        placeholder="C:/Games/QmClient"
                        className="h-12 min-w-0 rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#9a9fa8] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
                      />
                      <button
                        type="button"
                        onClick={() => void props.onBrowse()}
                        className="h-12 rounded-[18px] border border-[var(--dm-border)] bg-white px-5 text-sm font-black text-[var(--dm-ink)] transition hover:-translate-y-0.5 hover:bg-[var(--dm-paper)]"
                      >
                        定位
                      </button>
                      <button
                        type="button"
                        onClick={() => void props.onValidate()}
                        disabled={!canValidate}
                        className="h-12 rounded-[18px] bg-[var(--dm-ink)] px-5 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
                      >
                        验证
                      </button>
                    </div>
                    <div className="mt-4 border-t border-[var(--dm-border)] pt-4">
                      <div className="text-[10px] font-black uppercase tracking-[0.22em] text-[var(--dm-muted-ink)]">背景</div>
                      <div className="mt-2 flex flex-wrap gap-2">
                        <button
                          type="button"
                          onClick={props.onClearBackgroundImage}
                          className={`h-10 rounded-[16px] border px-4 text-xs font-black transition hover:-translate-y-0.5 ${
                            props.backgroundMode === "default"
                              ? "border-[var(--dm-ink)] bg-[var(--dm-ink)] text-white"
                              : "border-[var(--dm-border)] bg-white text-[var(--dm-muted-ink)]"
                          }`}
                        >
                          默认背景
                        </button>
                        <label className="grid h-10 cursor-pointer place-items-center rounded-[16px] border border-[var(--dm-border)] bg-white px-4 text-xs font-black text-[var(--dm-muted-ink)] transition hover:-translate-y-0.5 hover:bg-[var(--dm-paper)]">
                          自定义图片
                          <input
                            type="file"
                            accept="image/*"
                            className="hidden"
                            onChange={(event) => {
                              const file = event.target.files?.[0];
                              event.currentTarget.value = "";
                              if (file) {
                                void props.onBackgroundImageSelect(file);
                              }
                            }}
                          />
                        </label>
                      </div>
                    </div>
                  </div>
                </motion.div>
              </CollapsibleContent>
            ) : null}
          </AnimatePresence>

          {props.errorMessage ? (
            <div className="mt-4 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">
              {props.errorMessage}
            </div>
          ) : null}
        </Collapsible>
        <div className="min-[1024px]:hidden">{props.mobileUpdateStatus}</div>
      </motion.div>
    </section>
  );
}
