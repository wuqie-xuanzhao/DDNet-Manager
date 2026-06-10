import { getCurrentWindow } from "@tauri-apps/api/window";
import { Hexagon, Minus, X } from "lucide-react";

function currentWindow() {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

type TitleBarProps = {
  onOpenSettings: () => void;
};

function minimizeWindow() {
  void currentWindow()?.minimize();
}

function closeWindow() {
  void currentWindow()?.close();
}

export function TitleBar(props: TitleBarProps) {
  return (
    <div
      data-tauri-drag-region
      className="pointer-events-auto absolute left-0 top-0 z-50 flex h-14 w-full cursor-default select-none items-start justify-end px-5 pt-3"
    >
      <div className="flex items-center gap-3" data-tauri-drag-region>
        <button
          type="button"
          aria-label="Open settings"
          onClick={props.onOpenSettings}
          className="group relative grid h-9 w-9 place-items-center rounded-[9px] bg-[#222437]/72 text-white/78 shadow-[0_9px_24px_rgba(0,0,0,0.24)] backdrop-blur-md transition duration-200 hover:-translate-y-0.5 hover:bg-[#2f324b]/86 hover:text-white"
        >
          <Hexagon size={18} strokeWidth={2.3} />
          <span className="pointer-events-none absolute right-[-36px] top-[48px] z-[100] translate-y-1 rounded-[8px] bg-[#f2f4f8] px-5 py-2 text-base font-bold text-[#34363d] opacity-0 shadow-[0_10px_24px_rgba(0,0,0,0.22)] transition duration-200 before:absolute before:-top-2 before:left-1/2 before:h-4 before:w-4 before:-translate-x-1/2 before:rotate-45 before:bg-[#f2f4f8] group-hover:translate-y-0 group-hover:opacity-100">
            设置
          </span>
        </button>
        <button
          type="button"
          aria-label="Minimize window"
          onClick={minimizeWindow}
          className="group relative grid h-9 w-9 place-items-center rounded-[9px] bg-transparent text-white/76 transition duration-200 hover:-translate-y-0.5 hover:bg-[#222437]/62 hover:text-white"
        >
          <Minus size={16} strokeWidth={1.8} />
          <span className="pointer-events-none absolute right-[-42px] top-[48px] z-[100] translate-y-1 rounded-[8px] bg-[#f2f4f8] px-5 py-2 text-base font-bold text-[#34363d] opacity-0 shadow-[0_10px_24px_rgba(0,0,0,0.22)] transition duration-200 before:absolute before:-top-2 before:left-1/2 before:h-4 before:w-4 before:-translate-x-1/2 before:rotate-45 before:bg-[#f2f4f8] group-hover:translate-y-0 group-hover:opacity-100">
            最小化
          </span>
        </button>
        <button
          type="button"
          aria-label="Close window"
          onClick={closeWindow}
          className="group relative grid h-9 w-9 place-items-center rounded-[9px] bg-transparent text-white/76 transition duration-200 hover:-translate-y-0.5 hover:bg-red-500/18 hover:text-white"
        >
          <X size={16} strokeWidth={1.8} />
          <span className="pointer-events-none absolute right-[-30px] top-[48px] z-[100] translate-y-1 rounded-[8px] bg-[#f2f4f8] px-5 py-2 text-base font-bold text-[#34363d] opacity-0 shadow-[0_10px_24px_rgba(0,0,0,0.22)] transition duration-200 before:absolute before:-top-2 before:left-1/2 before:h-4 before:w-4 before:-translate-x-1/2 before:rotate-45 before:bg-[#f2f4f8] group-hover:translate-y-0 group-hover:opacity-100">
            关闭
          </span>
        </button>
      </div>
    </div>
  );
}
