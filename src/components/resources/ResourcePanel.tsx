import { useRef, useState } from "react";
import { validateClientDir } from "../../lib/tauri";
import type { ClientInstallation } from "../../types";

type ResourceFieldProps = {
  label: string;
  value: string | null;
};

function ResourceField({ label, value }: ResourceFieldProps) {
  return (
    <div className="rounded-[20px] bg-white/78 p-3">
      <div className="text-[11px] font-black tracking-[0.16em] text-[#59606d]">{label}</div>
      <div className="mt-2 break-all text-sm font-semibold leading-6 text-[#2f3440]">{value ?? "未识别"}</div>
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
    <section className="rounded-[34px] border border-[var(--dm-border)] bg-white/82 p-6 text-[var(--dm-ink)] shadow-[0_24px_70px_rgba(47,52,64,0.10)] backdrop-blur-2xl">
      <h2 className="text-3xl font-black tracking-[-0.05em]">资源位置</h2>
      <p className="mt-2 text-sm font-semibold text-[#4f5663]">识别安装目录、资源目录和用户数据目录。</p>

      <div className="mt-5 rounded-[26px] bg-[var(--dm-soft)] p-4">
        <label className="block text-[11px] font-black tracking-[0.18em] text-[#59606d]" htmlFor="resource-dir-input">
          Client Directory
        </label>
        <input
          id="resource-dir-input"
          value={path}
          onChange={(event) => handlePathChange(event.target.value)}
          disabled={isValidating}
          className="mt-3 h-12 w-full rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
          placeholder="C:/Games/QmClient"
          spellCheck={false}
        />
        <div className="mt-4 flex flex-wrap items-center gap-3">
          <button
            onClick={() => void validate()}
            disabled={!path.trim() || isValidating}
            className="h-11 rounded-[18px] bg-[var(--dm-ink)] px-5 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
          >
            {isValidating ? "识别中" : "识别"}
          </button>
          <div className="text-xs font-bold text-[#59606d]">只读目录验证。</div>
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[240px_minmax(0,1fr)]">
        <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
          <div className="mb-3 flex items-center justify-between text-xs font-black text-[#59606d]">
            <span>Health</span>
            <span>{client?.health ?? "-"}</span>
          </div>
          <div className="grid gap-2 rounded-[20px] bg-white/76 p-3 text-sm font-bold text-[#3d4350]">
            <div className="flex items-center justify-between">
              <span>Client</span>
              <span>{client?.client_id ?? "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Display</span>
              <span>{client?.display_name ?? "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Default</span>
              <span>{client ? (client.is_default ? "yes" : "no") : "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Version</span>
              <span>{client?.version ?? "-"}</span>
            </div>
          </div>
        </div>

        <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
          <div className="mb-3 text-xs font-black text-[#59606d]">Resolved Paths</div>
          {client ? (
            <div className="dm-scroll grid max-h-[420px] gap-3 overflow-y-auto pr-1">
              <ResourceField label="Install Dir" value={client.install_dir} />
              <ResourceField label="Executable Path" value={client.executable_path} />
              <ResourceField label="Storage CFG Path" value={client.storage_cfg_path} />
              <ResourceField label="Data Dir" value={client.data_dir} />
              <ResourceField label="User Data Dir" value={client.user_data_dir} />
            </div>
          ) : (
            <div className="rounded-[20px] border border-dashed border-[var(--dm-border)] bg-white/64 px-4 py-6 text-sm font-semibold text-[#59606d]">
              输入客户端目录后可查看路径。
            </div>
          )}
        </div>
      </div>

      {error ? <div className="mt-4 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">{error}</div> : null}
    </section>
  );
}
