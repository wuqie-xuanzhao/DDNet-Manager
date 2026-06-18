import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, ExternalLink, Loader2, AlertCircle, HardDrive } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter
} from "@/components/ui/dialog";
import { Checkbox } from "@/components/ui/checkbox";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Button } from "@/components/ui/button";
import { probeDisk } from "@/lib/tauri";
import type { DiskProbe } from "@/types";
import type { useClientInstaller } from "@/hooks/useClientInstaller";

interface InstallDialogProps {
  installer: ReturnType<typeof useClientInstaller>;
  /// 客户端显示名（从 catalog 拉，如 "QmClient" / "DDNet"）
  displayName: string;
  /// game id（用于生成默认安装路径，如 "qmclient"）
  gameId: string;
}

/// 默认安装路径：启动器自管理目录下的 clients/<game_id>/v<version>/。
/// 用平台特定路径分隔符。版本号在弹窗打开时根据 release 数据填入。
function buildDefaultInstallDir(gameId: string, latestVersion: string | null): string {
  const isWindows = navigator.userAgent.includes("Windows");
  const sep = isWindows ? "\\" : "/";
  const base = isWindows ? `%LOCALAPPDATA%${sep}DDNetManager${sep}clients` : `~${sep}.local${sep}share${sep}DDNetManager${sep}clients`;
  const version = latestVersion ?? "latest";
  return `${base}${sep}${gameId}${sep}v${version}`;
}

/// 格式化字节数为人类可读（MB / GB）。
function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

export function InstallDialog({ installer, displayName, gameId }: InstallDialogProps) {
  const { state, installDialogOpen, installDialogMode, closeInstallDialog, beginInstall, triggerScan, scanning } = installer;

  // 从 state 提取 release 元数据（installed 状态时 latest/assetSize/releaseUrl 可用）。
  // 否则 release 尚未拉到，弹窗显示加载状态。
  const releaseInfo = state.kind === "installed" ? state : null;
  const latestVersion = releaseInfo?.latest ?? null;
  const assetSize = releaseInfo?.assetSize ?? null;
  const releaseUrl = releaseInfo?.releaseUrl ?? null;

  const [installDir, setInstallDir] = useState("");
  const [desktopShortcut, setDesktopShortcut] = useState(true);
  const [startMenuShortcut, setStartMenuShortcut] = useState(true);
  const [macosMode, setMacosMode] = useState<"managed" | "replace">("managed");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [diskProbe, setDiskProbe] = useState<DiskProbe | null>(null);
  const [diskProbing, setDiskProbing] = useState(false);
  const [diskProbeError, setDiskProbeError] = useState<string | null>(null);

  // 弹窗打开时初始化默认路径。version 变了也重新计算（首次拉到 release 后）。
  useEffect(() => {
    if (installDialogOpen) {
      setInstallDir(buildDefaultInstallDir(gameId, latestVersion));
      setError(null);
    }
  }, [installDialogOpen, gameId, latestVersion]);

  // 安装路径变化时探测磁盘信息（剩余空间 + SSD/HDD）。debounce 避免每键打 IPC。
  useEffect(() => {
    if (!installDialogOpen || !installDir.trim()) {
      setDiskProbe(null);
      return;
    }
    let cancelled = false;
    setDiskProbing(true);
    setDiskProbeError(null);
    const timer = setTimeout(() => {
      void probeDisk(installDir)
        .then((probe) => {
          if (cancelled) return;
          setDiskProbe(probe);
        })
        .catch((err) => {
          if (cancelled) return;
          setDiskProbe(null);
          setDiskProbeError(err instanceof Error ? err.message : String(err));
        })
        .finally(() => {
          if (cancelled) return;
          setDiskProbing(false);
        });
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [installDialogOpen, installDir]);

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "选择安装目录" });
      if (typeof selected === "string") {
        setInstallDir(selected);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleStartInstall = async () => {
    const trimmed = installDir.trim();
    if (!trimmed) {
      setError("请选择安装目录");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await beginInstall({
        installDir: trimmed,
        desktop: desktopShortcut,
        startMenu: startMenuShortcut
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const isUpdate = installDialogMode === "update";
  const title = isUpdate ? `更新 ${displayName}` : `安装 ${displayName}`;

  return (
    <Dialog open={installDialogOpen} onOpenChange={(open) => { if (!open) closeInstallDialog(); }}>
      <DialogContent className="sm:max-w-[520px] bg-[var(--app-surface)] border-[var(--app-border)] text-[var(--app-text)]">
        <DialogHeader>
          <DialogTitle className="text-base font-bold text-[var(--app-text)]">{title}</DialogTitle>
          <DialogDescription className="text-xs text-[var(--app-text-muted)]">
            {isUpdate ? "下载并安装最新版本到当前客户端目录" : "选择安装位置、快捷方式等选项后开始下载"}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* 版本信息 */}
          <div className="rounded-lg border border-[var(--app-border-subtle)] bg-[var(--app-sunken)] p-3 space-y-1.5">
            <div className="text-[10px] font-bold uppercase tracking-wider text-[var(--app-text-muted)]">版本信息</div>
            {latestVersion ? (
              <div className="flex items-center justify-between text-xs">
                <div className="flex items-center gap-2">
                  <span className="font-bold text-[var(--app-text)]">v{latestVersion}</span>
                  <span className="text-[var(--app-text-dim)]">{isUpdate ? "最新" : "(最新)"}</span>
                </div>
                <div className="flex items-center gap-3 text-[var(--app-text-muted)] font-mono">
                  {assetSize != null && <span>{formatBytes(assetSize)}</span>}
                  {releaseUrl && (
                    <a
                      href={releaseUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1 text-[var(--app-accent)] hover:text-[var(--app-accent-hover)] font-bold transition-colors"
                    >
                      GitHub Release
                      <ExternalLink className="w-3 h-3" />
                    </a>
                  )}
                </div>
              </div>
            ) : (
              <div className="flex items-center gap-2 text-xs text-[var(--app-text-muted)] py-1">
                <Loader2 className="w-3 h-3 animate-spin" />
                正在获取版本信息…
              </div>
            )}
          </div>

          {/* 安装位置 */}
          <div className="space-y-1.5">
            <div className="text-[10px] font-bold uppercase tracking-wider text-[var(--app-text-muted)]">安装位置</div>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={installDir}
                onChange={(e) => setInstallDir(e.target.value)}
                spellCheck={false}
                className="flex-1 bg-[var(--app-sunken)] border border-[var(--app-border-subtle)] rounded-lg px-3 py-2 text-xs font-mono text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-accent)] transition-colors"
                placeholder="选择或输入安装目录"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => void handleBrowse()}
                className="border-[var(--app-border)] text-[var(--app-text-secondary)] hover:text-[var(--app-text)]"
              >
                <Folder className="w-3.5 h-3.5 mr-1" />
                更改
              </Button>
            </div>
            {/* SSD/HDD 标签 + 剩余空间 */}
            <div className="flex items-center gap-2 text-[10px] text-[var(--app-text-dim)] mt-1">
              {diskProbing ? (
                <>
                  <Loader2 className="w-3 h-3 animate-spin" />
                  <span>探测磁盘信息中…</span>
                </>
              ) : diskProbe ? (
                <>
                  <HardDrive className="w-3 h-3" />
                  <span className="font-mono uppercase tracking-wide">
                    {diskProbe.is_ssd === true ? "SSD" : diskProbe.is_ssd === false ? "HDD" : "未知磁盘类型"}
                  </span>
                  <span>·</span>
                  <span className="font-mono">
                    剩余 {formatBytes(diskProbe.free_bytes)} / {formatBytes(diskProbe.total_bytes)}
                  </span>
                  {diskProbe.is_ssd === false && (
                    <span className="text-amber-400/80">· 建议安装在 SSD</span>
                  )}
                </>
              ) : diskProbeError ? (
                <span className="text-[var(--app-text-dim)]">磁盘信息不可用</span>
              ) : (
                <span>建议安装在 SSD 上以获得更快的启动和加载速度</span>
              )}
            </div>
          </div>

          {/* 快捷方式 */}
          {!isUpdate && (
            <div className="space-y-2">
              <div className="text-[10px] font-bold uppercase tracking-wider text-[var(--app-text-muted)]">快捷方式</div>
              <label className="flex items-center gap-2.5 cursor-pointer py-1">
                <Checkbox
                  checked={desktopShortcut}
                  onCheckedChange={(v) => setDesktopShortcut(Boolean(v))}
                />
                <span className="text-xs text-[var(--app-text-secondary)]">创建桌面快捷方式</span>
              </label>
              <label className="flex items-center gap-2.5 cursor-pointer py-1">
                <Checkbox
                  checked={startMenuShortcut}
                  onCheckedChange={(v) => setStartMenuShortcut(Boolean(v))}
                />
                <span className="text-xs text-[var(--app-text-secondary)]">创建开始菜单快捷方式</span>
              </label>
            </div>
          )}

          {/* macOS 管理模式：仅在 macOS 显示。当前平台不影响，保留 UI 但 condition 控制 */}
          {navigator.userAgent.includes("Macintosh") && !isUpdate && (
            <div className="space-y-2">
              <div className="text-[10px] font-bold uppercase tracking-wider text-[var(--app-text-muted)]">macOS 管理模式</div>
              <RadioGroup
                value={macosMode}
                onValueChange={(v) => setMacosMode(v as "managed" | "replace")}
                className="space-y-2"
              >
                <label className="flex items-start gap-2.5 cursor-pointer">
                  <RadioGroupItem value="managed" id="mac-managed" />
                  <div>
                    <div className="text-xs font-medium text-[var(--app-text-secondary)]">在本启动器内独立管理（推荐）</div>
                    <div className="text-[10px] text-[var(--app-text-dim)] mt-0.5 leading-relaxed">
                      装到 ~/Library/Application Support/DDNetManager/clients/DDNet.app
                    </div>
                  </div>
                </label>
                <label className="flex items-start gap-2.5 cursor-pointer">
                  <RadioGroupItem value="replace" id="mac-replace" />
                  <div>
                    <div className="text-xs font-medium text-[var(--app-text-secondary)]">替换 /Applications/DDNet.app</div>
                    <div className="text-[10px] text-[var(--app-text-dim)] mt-0.5 leading-relaxed">
                      若已存在则备份后替换
                    </div>
                  </div>
                </label>
              </RadioGroup>
            </div>
          )}

          {error && (
            <div className="flex items-start gap-2 text-xs text-red-400 bg-[var(--app-danger-subtle)] border border-[var(--app-danger-border)] rounded-lg px-3 py-2">
              <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
              <span className="leading-relaxed">{error}</span>
            </div>
          )}
        </div>

        <DialogFooter className="flex-row sm:justify-between items-center gap-2 border-t border-[var(--app-border-subtle)] -mx-4 -mb-4 px-4 py-3 bg-[var(--app-surface-alt)] rounded-b-xl">
          {/* 左下：已安装？定位游戏 */}
          <button
            type="button"
            onClick={() => void triggerScan()}
            disabled={scanning}
            className="text-[11px] text-[var(--app-text-muted)] hover:text-[var(--app-accent)] font-bold transition-colors disabled:opacity-50"
          >
            {scanning ? "扫描中…" : "已安装？定位游戏"}
          </button>

          {/* 右下：开始安装 / 更新 */}
          <Button
            type="button"
            onClick={() => void handleStartInstall()}
            disabled={submitting || !installDir.trim() || !latestVersion}
            className="bg-[var(--app-accent)] text-[var(--app-accent-foreground)] hover:bg-[var(--app-accent-hover)] font-bold"
          >
            {submitting ? (
              <>
                <Loader2 className="w-3.5 h-3.5 mr-1.5 animate-spin" />
                提交中…
              </>
            ) : isUpdate ? (
              "开始更新"
            ) : (
              "开始安装"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
