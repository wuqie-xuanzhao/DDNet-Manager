import cloudDownloadIcon from "@/assets/game-icons/cloud-download.svg?raw";
import folderIcon from "@/assets/game-icons/folder.svg?raw";
import gamepadIcon from "@/assets/game-icons/gamepad.svg?raw";
import keyboardIcon from "@/assets/game-icons/keyboard.svg?raw";
import playIcon from "@/assets/game-icons/play.svg?raw";
import refreshIcon from "@/assets/game-icons/refresh.svg?raw";
import settingsIcon from "@/assets/game-icons/settings.svg?raw";
import toolKitIcon from "@/assets/game-icons/tool-kit.svg?raw";
import wrenchIcon from "@/assets/game-icons/wrench.svg?raw";

import { cn } from "@/lib/utils";

const iconUrls = {
  cloudDownload: cloudDownloadIcon,
  folder: folderIcon,
  gamepad: gamepadIcon,
  keyboard: keyboardIcon,
  play: playIcon,
  refresh: refreshIcon,
  settings: settingsIcon,
  toolKit: toolKitIcon,
  wrench: wrenchIcon
} as const;

export type GameIconName = keyof typeof iconUrls;

type GameIconProps = {
  name: GameIconName;
  className?: string;
};

export function GameIcon({ name, className }: GameIconProps) {
  return (
    <span
      data-game-icon={name}
      aria-hidden
      className={cn("inline-block size-5 shrink-0 [&_svg]:h-full [&_svg]:w-full [&_svg]:overflow-visible", className)}
      dangerouslySetInnerHTML={{ __html: normalizedIconSvg(iconUrls[name]) }}
    />
  );
}

function normalizedIconSvg(svg: string) {
  return svg
    .replaceAll('fill="white"', 'fill="currentColor"')
    .replace("<svg ", '<svg focusable="false" ');
}
