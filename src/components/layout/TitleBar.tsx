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
      className="pointer-events-none absolute left-0 top-0 z-50 flex h-12 w-full items-center justify-end px-5"
    >
      <div className="pointer-events-auto flex h-full items-center gap-2 pt-2">
        <button
          type="button"
          aria-label="Open settings"
          onClick={props.onOpenSettings}
          className="group relative grid h-12 w-12 place-items-center rounded-[14px] bg-transparent text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.42)] transition duration-200 hover:-translate-y-0.5 hover:bg-[#111213]/76 hover:shadow-[0_14px_28px_rgba(17,18,19,0.22)] hover:backdrop-blur-md"
        >
          <Hexagon size={18} strokeWidth={2.3} />
          <span className="pointer-events-none absolute right-1 top-[56px] translate-y-1 rounded-xl bg-white px-4 py-2 text-sm font-bold text-[#2f3440] opacity-0 shadow-xl transition duration-200 before:absolute before:-top-1.5 before:right-5 before:h-3 before:w-3 before:rotate-45 before:bg-white group-hover:translate-y-0 group-hover:opacity-100">
            设置
          </span>
        </button>
        <button
          type="button"
          aria-label="Minimize window"
          onClick={minimizeWindow}
          className="group relative grid h-12 w-12 place-items-center rounded-[14px] bg-transparent text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.42)] transition duration-200 hover:-translate-y-0.5 hover:bg-[#111213]/76 hover:shadow-[0_14px_28px_rgba(17,18,19,0.22)] hover:backdrop-blur-md"
        >
          <Minus size={16} strokeWidth={1.8} />
          <span className="pointer-events-none absolute right-1 top-[56px] translate-y-1 rounded-xl bg-white px-4 py-2 text-sm font-bold text-[#2f3440] opacity-0 shadow-xl transition duration-200 before:absolute before:-top-1.5 before:right-5 before:h-3 before:w-3 before:rotate-45 before:bg-white group-hover:translate-y-0 group-hover:opacity-100">
            最小化
          </span>
        </button>
        <button
          type="button"
          aria-label="Close window"
          onClick={closeWindow}
          className="group relative grid h-12 w-12 place-items-center rounded-[14px] bg-transparent text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.42)] transition duration-200 hover:-translate-y-0.5 hover:bg-[#111213]/76 hover:shadow-[0_14px_28px_rgba(17,18,19,0.22)] hover:backdrop-blur-md"
        >
          <X size={16} strokeWidth={1.8} />
          <span className="pointer-events-none absolute right-1 top-[56px] translate-y-1 rounded-xl bg-white px-4 py-2 text-sm font-bold text-[#2f3440] opacity-0 shadow-xl transition duration-200 before:absolute before:-top-1.5 before:right-5 before:h-3 before:w-3 before:rotate-45 before:bg-white group-hover:translate-y-0 group-hover:opacity-100">
            关闭
          </span>
        </button>
      </div>
    </div>
  );
}
