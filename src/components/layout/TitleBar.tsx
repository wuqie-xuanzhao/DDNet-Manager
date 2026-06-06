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
      className="absolute left-0 top-0 z-50 flex h-11 w-full items-center justify-between border-b border-white/10 bg-[#151827]/92 px-4 shadow-[0_12px_36px_rgba(5,8,18,0.28)] backdrop-blur-2xl"
    >
      <div data-tauri-drag-region className="flex items-center gap-3 text-[11px] font-black uppercase tracking-[0.32em] text-white/78">
        <div className="h-2.5 w-2.5 rounded-full bg-[#ff8fbd] shadow-[0_0_18px_rgba(255,143,189,0.85)]" />
        DDNet Manager
        <span className="hidden rounded-full bg-[#ff8fbd]/14 px-2.5 py-1 text-[10px] tracking-[0.18em] text-[#ffb8d6] sm:inline">
          Candy Utility
        </span>
      </div>

      <div className="flex h-full items-center">
        <button
          aria-label="Minimize window"
          onClick={minimize}
          className="grid h-11 w-12 place-items-center text-white/58 transition hover:bg-[#8bdcff]/16 hover:text-[#c9f2ff]"
        >
          <Minus size={16} strokeWidth={1.8} />
        </button>
        <button
          aria-label="Close window"
          onClick={close}
          className="grid h-11 w-12 place-items-center text-white/58 transition hover:bg-[#ff8fbd]/22 hover:text-[#ffd9e9]"
        >
          <X size={16} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
