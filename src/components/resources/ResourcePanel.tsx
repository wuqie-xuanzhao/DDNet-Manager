import { useRef, useState } from "react";
import { validateClientDir } from "../../lib/tauri";
import type { ClientInstallation } from "../../types";

type ResourceFieldProps = {
  label: string;
  value: string | null;
  accent?: "default" | "cyan";
};

function ResourceField({ label, value, accent = "default" }: ResourceFieldProps) {
  return (
    <div className="border border-white/8 bg-black/20 p-3">
      <div className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{label}</div>
      <div className={`mt-2 break-all text-sm leading-6 ${accent === "cyan" ? "text-cyan-100" : "text-slate-200"}`}>
        {value ?? "未识别"}
      </div>
    </div>
  );
}

export function ResourcePanel() {
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
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Resources</div>
      <h2 className="mt-3 text-2xl font-black">资源位置</h2>
      <p className="mt-2 text-sm text-slate-300/80">
        基于现有目录验证命令展示安装目录、资源目录和用户数据目录的识别结果。
      </p>

      <div className="mt-5 border border-white/10 bg-black/20 p-4">
        <label className="block text-[10px] font-black uppercase tracking-[0.28em] text-slate-400" htmlFor="resource-dir-input">
          Client Directory
        </label>
        <input
          id="resource-dir-input"
          value={path}
          onChange={(event) => handlePathChange(event.target.value)}
          disabled={isValidating}
          className="mt-3 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-200/50"
          placeholder="C:/Games/QmClient"
          spellCheck={false}
        />
        <div className="mt-3 flex flex-wrap items-center gap-3">
          <button
            onClick={() => void validate()}
            disabled={!path.trim() || isValidating}
            className="border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-slate-950 transition disabled:cursor-not-allowed disabled:border-cyan-200/20 disabled:bg-cyan-200/20 disabled:text-cyan-100/55"
          >
            {isValidating ? "识别中..." : "识别资源位置"}
          </button>
          <div className="text-xs uppercase tracking-[0.18em] text-slate-500">Readonly Directory Validation / No Explorer Action</div>
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[220px_minmax(0,1fr)]">
        <div className="border border-cyan-200/12 bg-black/30 p-3">
          <div className="mb-3 flex items-center justify-between text-[10px] font-black uppercase tracking-[0.28em] text-cyan-100/72">
            <span>Health</span>
            <span>{client?.health ?? "-"}</span>
          </div>
          <div className="grid gap-2 border border-white/6 bg-black/20 p-3 text-[11px] uppercase tracking-[0.18em] text-slate-400">
            <div className="flex items-center justify-between">
              <span>Client</span>
              <span className="text-cyan-100">{client?.client_id ?? "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Display</span>
              <span className="text-cyan-100">{client?.display_name ?? "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Default</span>
              <span className="text-cyan-100">{client ? (client.is_default ? "yes" : "no") : "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Version</span>
              <span className="text-cyan-100">{client?.version ?? "-"}</span>
            </div>
          </div>
        </div>

        <div className="border border-white/8 bg-black/25 p-3">
          <div className="mb-3 text-[10px] font-black uppercase tracking-[0.28em] text-slate-300/80">Resolved Paths</div>
          {client ? (
            <div className="grid max-h-[420px] gap-3 overflow-y-auto pr-1">
              <ResourceField label="Install Dir" value={client.install_dir} accent="cyan" />
              <ResourceField label="Executable Path" value={client.executable_path} />
              <ResourceField label="Storage CFG Path" value={client.storage_cfg_path} />
              <ResourceField label="Data Dir" value={client.data_dir} />
              <ResourceField label="User Data Dir" value={client.user_data_dir} />
            </div>
          ) : (
            <div className="border border-dashed border-white/10 bg-black/20 px-4 py-6 text-sm text-slate-400">
              输入客户端目录后可查看安装路径、资源路径和用户数据路径的识别结果。
            </div>
          )}
        </div>
      </div>

      {error ? <div className="mt-4 text-sm text-red-300">{error}</div> : null}
    </section>
  );
}
