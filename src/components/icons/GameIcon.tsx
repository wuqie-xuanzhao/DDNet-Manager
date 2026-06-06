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

const iconSources = {
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

const coloredIconSources = Object.fromEntries(
  Object.entries(iconSources).map(([name, svg]) => [
    name,
    svg
      .replaceAll('fill="white"', 'fill="currentColor"')
      .replaceAll('fill="#FFFFFF"', 'fill="currentColor"')
      .replace("<svg ", '<svg aria-hidden="true" focusable="false" ')
  ])
) as typeof iconSources;

export type GameIconName = keyof typeof iconSources;

type GameIconProps = {
  name: GameIconName;
  className?: string;
};

export function GameIcon({ name, className }: GameIconProps) {
  return (
    <span
      aria-hidden
      className={cn("inline-grid size-5 shrink-0 place-items-center text-current [&_svg]:block [&_svg]:size-full", className)}
      dangerouslySetInnerHTML={{ __html: coloredIconSources[name] }}
    />
  );
}
