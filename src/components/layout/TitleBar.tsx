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
      className="absolute left-0 top-0 z-50 flex h-11 w-full items-center justify-between border-b border-white/60 bg-white/55 px-4 backdrop-blur-2xl"
    >
      <div data-tauri-drag-region className="flex items-center gap-3 text-[11px] font-black uppercase tracking-[0.32em] text-[#7d86a6]">
        <div className="h-2.5 w-2.5 rounded-full bg-[#ff8fbd] shadow-[0_0_18px_rgba(255,143,189,0.55)]" />
        DDNet Manager
      </div>

      <div className="flex h-full items-center">
        <button
          aria-label="Minimize window"
          onClick={minimize}
          className="grid h-11 w-12 place-items-center text-[#8b95ad] transition hover:bg-[#dff7ff]/70 hover:text-[#4a95ba]"
        >
          <Minus size={16} strokeWidth={1.8} />
        </button>
        <button
          aria-label="Close window"
          onClick={close}
          className="grid h-11 w-12 place-items-center text-[#8b95ad] transition hover:bg-[#ffe1ee] hover:text-[#d65e8d]"
        >
          <X size={16} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
