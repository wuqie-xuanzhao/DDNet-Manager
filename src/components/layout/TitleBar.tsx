import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Settings, X } from "lucide-react";

const appWindow = getCurrentWindow();

type TitleBarProps = {
  onOpenSettings: () => void;
};

export function TitleBar(props: TitleBarProps) {
  const minimize = () => {
    void appWindow.minimize();
  };

  const close = () => {
    void appWindow.close();
  };

  return (
    <div
      data-tauri-drag-region
      className="absolute left-0 top-0 z-50 flex h-11 w-full items-center justify-between border-b border-[var(--dm-border)] bg-[var(--dm-paper)]/92 px-4 shadow-[0_10px_30px_rgba(47,52,64,0.08)] backdrop-blur-2xl"
    >
      <div data-tauri-drag-region className="flex items-center gap-3 text-[11px] font-black uppercase tracking-[0.28em] text-[var(--dm-ink)]">
        <div className="h-2.5 w-2.5 rounded-full bg-[var(--dm-ink)]" />
        DDNet Manager
      </div>

      <div className="flex h-full items-center">
        <button
          aria-label="Open settings"
          onClick={props.onOpenSettings}
          className="grid h-11 w-12 place-items-center text-[var(--dm-muted-ink)] transition hover:bg-[var(--dm-soft)] hover:text-[var(--dm-ink)]"
        >
          <Settings size={16} strokeWidth={1.8} />
        </button>
        <button
          aria-label="Minimize window"
          onClick={minimize}
          className="grid h-11 w-12 place-items-center text-[var(--dm-muted-ink)] transition hover:bg-[var(--dm-soft)] hover:text-[var(--dm-ink)]"
        >
          <Minus size={16} strokeWidth={1.8} />
        </button>
        <button
          aria-label="Close window"
          onClick={close}
          className="grid h-11 w-12 place-items-center text-[#6d7280] transition hover:bg-[#b84a4a]/12 hover:text-[#8f2f2f]"
        >
          <X size={16} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
