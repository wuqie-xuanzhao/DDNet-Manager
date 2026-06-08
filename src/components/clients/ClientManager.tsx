import { useEffect, useRef, useState } from "react";
import {
  listClientInstallations,
  removeClientInstallation,
  scanClientInstallations,
  setDefaultClient,
  upsertClientInstallation,
  validateClientDir
} from "../../lib/tauri";
import type { ClientInstallation } from "../../types";

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
  const latestRequestIdRef = useRef(0);

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
    if (!nextPath) {
      return;
    }

    const requestId = latestRequestIdRef.current + 1;
    latestRequestIdRef.current = requestId;
    setError(null);
    setIsBusy(true);

    try {
      const nextClient = await validateClientDir(nextPath);
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
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
      if (latestRequestIdRef.current === requestId) {
        setIsBusy(false);
      }
    }
  };

  const scan = async () => {
    setError(null);
    setIsBusy(true);
    try {
      const results = await scanClientInstallations({ include_saved_paths: true });
      setCandidates(results);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  const saveCandidate = async (client: ClientInstallation) => {
    setError(null);
    setIsBusy(true);
    try {
      await upsertClientInstallation({
        install_dir: client.install_dir,
        is_default: clients.length === 0
      });
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
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">客户端</div>
      <h2 className="mt-3 text-2xl font-black">本地客户端</h2>
      <p className="mt-2 text-sm text-slate-300/80">扫描、手动添加并管理 QmClient / DDNet / 第三方客户端。</p>

      <div className="mt-5 border border-white/10 bg-black/20 p-4">
        <label className="block text-[10px] font-black uppercase tracking-[0.28em] text-slate-400" htmlFor="client-dir-input">
          客户端目录
        </label>
        <input
          id="client-dir-input"
          value={path}
          onChange={(event) => handlePathChange(event.target.value)}
          disabled={isBusy}
          className="mt-3 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-200/50"
          placeholder="C:/Games/QmClient"
          spellCheck={false}
        />
        <div className="mt-3 flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={() => void validateAndSave()}
            disabled={!path.trim() || isBusy}
            className="border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-cyan-950 transition disabled:cursor-not-allowed disabled:border-cyan-200/20 disabled:bg-cyan-200/20 disabled:text-cyan-100/55"
          >
            {isBusy ? "请稍候..." : "验证并保存"}
          </button>
          <button
            type="button"
            onClick={() => void scan()}
            disabled={isBusy}
            className="border border-white/15 bg-white/8 px-4 py-2 text-sm font-black text-slate-100 transition hover:border-cyan-200/35 disabled:cursor-not-allowed disabled:opacity-45"
          >
            扫描常见路径
          </button>
          <div className="text-xs text-slate-500">目录需要包含 DDNet.exe、storage.cfg 和 data。</div>
        </div>
      </div>

      <div className="mt-5 grid gap-4 xl:grid-cols-2">
        <div className="border border-white/10 bg-black/20 p-4">
          <div className="text-[10px] font-black uppercase tracking-[0.28em] text-slate-400">已保存</div>
          <div className="mt-3 space-y-3">
            {clients.length > 0 ? (
              clients.map((client) => (
                <article key={client.id} className="border border-cyan-200/12 bg-black/30 p-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-black text-cyan-100">
                        {client.display_name} {client.is_default ? "· 默认" : ""}
                      </div>
                      <div className="mt-1 truncate text-xs text-slate-400">{client.install_dir}</div>
                    </div>
                    <span className="shrink-0 border border-white/10 px-2 py-1 text-[10px] font-black uppercase text-slate-300">
                      {healthLabel(client)}
                    </span>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <button
                      type="button"
                      onClick={() => void makeDefault(client.id)}
                      disabled={client.is_default || isBusy}
                      className="border border-white/12 px-3 py-1.5 text-xs font-black text-slate-100 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      设为默认
                    </button>
                    <button
                      type="button"
                      onClick={() => void remove(client.id)}
                      disabled={isBusy}
                      className="border border-red-300/20 px-3 py-1.5 text-xs font-black text-red-200 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      移除记录
                    </button>
                  </div>
                </article>
              ))
            ) : (
              <div className="border border-dashed border-white/10 px-3 py-6 text-sm text-slate-500">还没有保存客户端。</div>
            )}
          </div>
        </div>

        <div className="border border-white/10 bg-black/20 p-4">
          <div className="text-[10px] font-black uppercase tracking-[0.28em] text-slate-400">扫描结果</div>
          <div className="mt-3 space-y-3">
            {candidates.length > 0 ? (
              candidates.map((client) => (
                <article key={client.id} className="border border-cyan-200/12 bg-black/30 p-3">
                  <div className="text-sm font-black text-cyan-100">{client.display_name}</div>
                  <div className="mt-1 break-all text-xs text-slate-400">{client.install_dir}</div>
                  <button
                    type="button"
                    onClick={() => void saveCandidate(client)}
                    disabled={isBusy}
                    className="mt-3 border border-cyan-200/35 px-3 py-1.5 text-xs font-black text-cyan-100 disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    保存此客户端
                  </button>
                </article>
              ))
            ) : (
              <div className="border border-dashed border-white/10 px-3 py-6 text-sm text-slate-500">扫描后会在这里列出找到的客户端。</div>
            )}
          </div>
        </div>
      </div>

      {error ? <div className="mt-4 text-sm text-red-300">{error}</div> : null}
    </section>
  );
}
