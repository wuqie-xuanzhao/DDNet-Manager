import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, X } from "lucide-react";

const appWindow = getCurrentWindow();

export function TitleBar() {
  const minimize = () => {
    void appWindow.minimize();
  };

  const close = () => {
    void appWindow.close();
  };

  return (
    <div
      data-tauri-drag-region
      className="absolute left-0 top-0 z-50 flex h-11 w-full items-center justify-between border-b border-white/8 bg-black/20 px-4 backdrop-blur-md"
    >
      <div data-tauri-drag-region className="flex items-center gap-3 text-[11px] font-semibold uppercase tracking-[0.38em] text-cyan-100/70">
        <div className="h-2 w-2 rounded-full bg-cyan-300 shadow-[0_0_18px_rgba(65,242,255,0.95)]" />
        DDNet Manager
      </div>

      <div className="flex h-full items-center">
        <button
          aria-label="Minimize window"
          onClick={minimize}
          className="grid h-11 w-12 place-items-center text-slate-300/80 transition hover:bg-white/8 hover:text-cyan-100"
        >
          <Minus size={16} strokeWidth={1.8} />
        </button>
        <button
          aria-label="Close window"
          onClick={close}
          className="grid h-11 w-12 place-items-center text-slate-300/80 transition hover:bg-red-500/90 hover:text-white"
        >
          <X size={16} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
