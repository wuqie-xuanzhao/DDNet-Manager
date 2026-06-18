import { useEffect, useRef, useState } from "react";
import {
  listClientInstallations,
  removeClientInstallation,
  setDefaultClient,
  upsertClientInstallation,
  validateClientDir
} from "../../lib/tauri";
import type { ClientInstallation } from "../../types";
import { useClientScanner } from "../../hooks/useClientScanner";
import { describeScanEvent } from "../../lib/scanProgress";

function healthLabel(client: ClientInstallation) {
  switch (client.health) {
    case "ok":
      return "OK";
    case "missing_executable":
      return "缺 DDNet.exe";
    case "missing_storage_cfg":
      return "缺 storage.cfg";
    case "missing_data_dir":
      return "缺 data";
  }
}

export function ClientManager() {
  const [path, setPath] = useState("");
  const [clients, setClients] = useState<ClientInstallation[]>([]);
  const [candidates, setCandidates] = useState<ClientInstallation[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [showUnhealthy, setShowUnhealthy] = useState(false);
  const latestRequestIdRef = useRef(0);
  const scanner = useClientScanner();
  const combinedError = error ?? scanner.error;

  const reload = async () => {
    setClients(await listClientInstallations());
  };

  useEffect(() => {
    void reload().catch((err) => setError(err instanceof Error ? err.message : String(err)));
  }, []);

  const handlePathChange = (nextPath: string) => {
    latestRequestIdRef.current += 1;
    setPath(nextPath);
    setError(null);
  };

  const validateAndSave = async () => {
    const nextPath = path.trim();
    if (!nextPath) return;

    const requestId = latestRequestIdRef.current + 1;
    latestRequestIdRef.current = requestId;
    setError(null);
    setIsBusy(true);

    try {
      const nextClient = await validateClientDir(nextPath);
      if (latestRequestIdRef.current !== requestId) return;
      const savedClient = await upsertClientInstallation({
        install_dir: nextClient.install_dir,
        is_default: clients.length === 0
      });
      await reload();
      setPath(savedClient.install_dir);
    } catch (err) {
      if (latestRequestIdRef.current === requestId) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (latestRequestIdRef.current === requestId) setIsBusy(false);
    }
  };

  const scan = async () => {
    setError(null);
    try {
      const results = await scanner.start({
        options: { include_saved_paths: true, include_unhealthy: showUnhealthy }
      });
      setCandidates(results);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const browse = async () => {
    setError(null);
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const selected = await openDialog({
        filters: [{ name: "DDNet 客户端可执行文件", extensions: ["exe"] }],
        multiple: false
      });
      if (typeof selected === "string") {
        // 选择 DDNet.exe 后取其所在目录作为安装目录。
        const dir = selected.replace(/[/\\][^/\\]+$/, "");
        setPath(dir);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const saveCandidate = async (client: ClientInstallation) => {
    setError(null);
    setIsBusy(true);
    try {
      await upsertClientInstallation({ install_dir: client.install_dir, is_default: clients.length === 0 });
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  const makeDefault = async (id: string) => {
    setError(null);
    setIsBusy(true);
    try {
      await setDefaultClient(id);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  const remove = async (id: string) => {
    setError(null);
    setIsBusy(true);
    try {
      await removeClientInstallation(id);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="space-y-3">
        <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
          <span className="text-[var(--app-text-muted)] text-sm font-bold uppercase tracking-wider">客户端目录</span>
        </div>
        <input
          id="client-dir-input"
          value={path}
          onChange={(event) => handlePathChange(event.target.value)}
          disabled={isBusy}
          className="bg-[var(--app-input)] border border-[var(--app-border)] rounded-lg px-3.5 py-2 w-full text-sm text-[var(--app-text-secondary)] focus:outline-none focus:border-[var(--app-focus)] font-mono"
          placeholder="C:/Games/QmClient"
          spellCheck={false}
        />
        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={() => void validateAndSave()}
            disabled={!path.trim() || isBusy}
            className="px-4 py-2 rounded-lg bg-[var(--app-accent)] hover:bg-[var(--app-accent-hover)] text-[var(--app-accent-foreground)] text-sm font-bold cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isBusy ? "请稍候..." : "验证并保存"}
          </button>
          <button
            type="button"
            onClick={() => void browse()}
            disabled={isBusy}
            className="px-4 py-2 rounded-lg bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] border border-[var(--app-border-subtle)] text-sm font-semibold text-[var(--app-text-secondary)] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-40"
          >
            浏览…
          </button>
          <button
            type="button"
            onClick={() => void scan()}
            disabled={isBusy || scanner.scanning}
            className="px-4 py-2 rounded-lg bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] border border-[var(--app-border-subtle)] text-sm font-semibold text-[var(--app-text-secondary)] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-40"
          >
            {scanner.scanning ? `扫描中… 已找到 ${scanner.foundCount}` : "扫描常见路径"}
          </button>
          <label className="flex items-center gap-1.5 text-xs text-[var(--app-text-dim)] cursor-pointer select-none">
            <input
              type="checkbox"
              checked={showUnhealthy}
              onChange={(event) => setShowUnhealthy(event.target.checked)}
              disabled={isBusy || scanner.scanning}
              className="cursor-pointer"
            />
            显示残缺客户端
          </label>
          {scanner.scanning ? (
            <button
              type="button"
              onClick={() => void scanner.cancel()}
              className="px-4 py-2 rounded-lg bg-[var(--app-danger-subtle)] hover:bg-[var(--app-danger-border)] border border-[var(--app-danger-border)] text-sm font-semibold text-[var(--app-danger)] cursor-pointer transition-colors"
            >
              取消扫描
            </button>
          ) : null}
        </div>
      </div>

      {scanner.events.length > 0 ? (
        <div className="space-y-3">
          <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
            <span className="text-[var(--app-text-muted)] text-sm font-bold uppercase tracking-wider">
              扫描进度
            </span>
            <span className="text-xs text-[var(--app-text-dim)]">
              已找到 <span className="font-mono font-bold text-[var(--app-text)]">{scanner.foundCount}</span> 个候选
            </span>
          </div>
          <ul className="space-y-1 max-h-64 overflow-y-auto">
            {scanner.events.map((event, idx) => (
              <li
                key={idx}
                className="text-xs text-[var(--app-text-secondary)] font-mono leading-relaxed"
              >
                {describeScanEvent(event)}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      <div className="space-y-3">
        <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
          <span className="text-[var(--app-text-muted)] text-sm font-bold uppercase tracking-wider">已保存</span>
        </div>
        {clients.length > 0 ? (
          <div className="space-y-3">
            {clients.map((client) => (
              <article key={client.id} className="bg-[var(--app-sunken)] border border-[var(--app-border-subtle)] rounded-lg p-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-bold text-[var(--app-text)]">
                      {client.display_name} {client.is_default ? "· 默认" : ""}
                    </div>
                    <div className="mt-1 truncate text-xs text-[var(--app-text-dim)] font-mono">{client.install_dir}</div>
                  </div>
                  <span className="shrink-0 border border-[var(--app-border)] px-2 py-1 text-xs font-bold uppercase text-[var(--app-text-muted)] rounded">
                    {healthLabel(client)}
                  </span>
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => void makeDefault(client.id)}
                    disabled={client.is_default || isBusy}
                    className="px-3 py-1.5 rounded-lg bg-[var(--app-border-subtle)] hover:bg-[var(--app-border)] text-sm font-semibold text-[var(--app-text-secondary)] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    设为默认
                  </button>
                  <button
                    type="button"
                    onClick={() => void remove(client.id)}
                    disabled={isBusy}
                    className="px-3 py-1.5 rounded-lg bg-[var(--app-danger-subtle)] hover:bg-[var(--app-danger-border)] border border-[var(--app-danger-border)] text-sm font-semibold text-[var(--app-danger)] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    移除记录
                  </button>
                </div>
              </article>
            ))}
          </div>
        ) : (
          <div className="border border-dashed border-[var(--app-border)] rounded-lg px-3 py-6 text-sm text-[var(--app-text-dim)] text-center">暂无已保存的客户端</div>
        )}
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between border-b border-[var(--app-border-subtle)] pb-1">
          <span className="text-[var(--app-text-muted)] text-sm font-bold uppercase tracking-wider">扫描结果</span>
        </div>
        {candidates.length > 0 ? (
          <div className="space-y-3">
            {candidates.map((client) => (
              <article key={client.id} className="bg-[var(--app-sunken)] border border-[var(--app-border-subtle)] rounded-lg p-3">
                <div className="text-sm font-bold text-[var(--app-text)]">{client.display_name}</div>
                <div className="mt-1 break-all text-xs text-[var(--app-text-dim)] font-mono">{client.install_dir}</div>
                <button
                  type="button"
                  onClick={() => void saveCandidate(client)}
                  disabled={isBusy}
                  className="mt-3 px-3 py-1.5 rounded-lg bg-[var(--app-accent-subtle)] hover:bg-[var(--app-accent-border)] border border-[var(--app-accent-border)] text-sm font-semibold text-[var(--app-accent)] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-40"
                >
                  保存此客户端
                </button>
              </article>
            ))}
          </div>
        ) : (
          <div className="border border-dashed border-[var(--app-border)] rounded-lg px-3 py-6 text-sm text-[var(--app-text-dim)] text-center">
            未找到客户端？点击"扫描常见路径"，或"浏览…"手动选择本机的 DDNet.exe。
          </div>
        )}
      </div>

      {combinedError ? <div className="text-sm text-[var(--app-danger)] bg-[var(--app-danger-subtle)] border border-[var(--app-danger-border)] rounded-lg px-3 py-2">{combinedError}</div> : null}
    </div>
  );
}
