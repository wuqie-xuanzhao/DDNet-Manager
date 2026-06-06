import { useRef, useState } from "react";
import { validateClientDir } from "../../lib/tauri";
import type { ClientInstallation } from "../../types";

export function ClientManager() {
  const [path, setPath] = useState("");
  const [client, setClient] = useState<ClientInstallation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const latestRequestIdRef = useRef(0);

  const handlePathChange = (nextPath: string) => {
    latestRequestIdRef.current += 1;
    setPath(nextPath);
    setClient(null);
    setError(null);
    setIsValidating(false);
  };

  const validate = async () => {
    const nextPath = path.trim();
    if (!nextPath) {
      return;
    }

    const requestId = latestRequestIdRef.current + 1;
    latestRequestIdRef.current = requestId;

    setError(null);
    setClient(null);
    setIsValidating(true);

    try {
      const nextClient = await validateClientDir(nextPath);
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setClient(nextClient);
    } catch (err) {
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (latestRequestIdRef.current === requestId) {
        setIsValidating(false);
      }
    }
  };

  return (
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Clients</div>
      <h2 className="mt-3 text-2xl font-black">本地客户端</h2>
      <p className="mt-2 text-sm text-slate-300/80">扫描、手动添加并管理 QmClient / DDNet 客户端。</p>

      <div className="mt-5 border border-white/10 bg-black/20 p-4">
        <label className="block text-[10px] font-black uppercase tracking-[0.28em] text-slate-400" htmlFor="client-dir-input">
          Client Directory
        </label>
        <input
          id="client-dir-input"
          value={path}
          onChange={(event) => handlePathChange(event.target.value)}
          disabled={isValidating}
          className="mt-3 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-200/50"
          placeholder="C:/Games/QmClient"
          spellCheck={false}
        />
        <div className="mt-3 flex items-center gap-3">
          <button
            onClick={() => void validate()}
            disabled={!path.trim() || isValidating}
            className="border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-slate-950 transition disabled:cursor-not-allowed disabled:border-cyan-200/20 disabled:bg-cyan-200/20 disabled:text-cyan-100/55"
          >
            {isValidating ? "验证中..." : "验证目录"}
          </button>
          <div className="text-xs uppercase tracking-[0.18em] text-slate-500">DDNet.exe / storage.cfg / data</div>
        </div>
      </div>

      {client ? (
        <pre className="mt-4 overflow-auto border border-cyan-200/12 bg-black/30 p-3 text-xs leading-6 text-cyan-100">
          {JSON.stringify(client, null, 2)}
        </pre>
      ) : null}

      {error ? <div className="mt-4 text-sm text-red-300">{error}</div> : null}
    </section>
  );
}
