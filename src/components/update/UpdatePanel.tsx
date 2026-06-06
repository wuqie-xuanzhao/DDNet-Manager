import { useRef, useState } from "react";
import { loadManifest } from "../../lib/tauri";
import type { UpdateManifest } from "../../types";

const DEFAULT_MANIFEST_URL = "https://ddrace.cn/manifest.json";

function formatAssetSize(size: number) {
  if (size >= 1024 * 1024) {
    return `${(size / (1024 * 1024)).toFixed(2)} MB`;
  }

  if (size >= 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${size} B`;
}

export function UpdatePanel() {
  const [manifestUrl, setManifestUrl] = useState(DEFAULT_MANIFEST_URL);
  const [proxyBaseUrl, setProxyBaseUrl] = useState("");
  const [manifest, setManifest] = useState<UpdateManifest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const latestRequestIdRef = useRef(0);

  const handleManifestUrlChange = (nextUrl: string) => {
    latestRequestIdRef.current += 1;
    setManifestUrl(nextUrl);
    setManifest(null);
    setError(null);
    setIsLoading(false);
  };

  const handleProxyBaseUrlChange = (nextUrl: string) => {
    latestRequestIdRef.current += 1;
    setProxyBaseUrl(nextUrl);
    setManifest(null);
    setError(null);
    setIsLoading(false);
  };

  const load = async () => {
    const nextManifestUrl = manifestUrl.trim();
    const nextProxyBaseUrl = proxyBaseUrl.trim();
    if (!nextManifestUrl) {
      return;
    }

    const requestId = latestRequestIdRef.current + 1;
    latestRequestIdRef.current = requestId;

    setManifest(null);
    setError(null);
    setIsLoading(true);

    try {
      const nextManifest = await loadManifest(
        nextManifestUrl,
        nextProxyBaseUrl ? nextProxyBaseUrl : undefined
      );
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setManifest(nextManifest);
    } catch (err) {
      if (latestRequestIdRef.current !== requestId) {
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (latestRequestIdRef.current === requestId) {
        setIsLoading(false);
      }
    }
  };

  return (
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Update</div>
      <h2 className="mt-3 text-2xl font-black">Manifest 更新</h2>
      <p className="mt-2 text-sm text-slate-300/80">
        读取自维护 manifest，先打通元数据闭环。下载事务待后续接入。
      </p>

      <div className="mt-5 border border-white/10 bg-black/20 p-4">
        <label className="block text-[10px] font-black uppercase tracking-[0.28em] text-slate-400" htmlFor="manifest-url-input">
          Manifest URL
        </label>
        <input
          id="manifest-url-input"
          value={manifestUrl}
          onChange={(event) => handleManifestUrlChange(event.target.value)}
          disabled={isLoading}
          className="mt-3 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-200/50"
          placeholder={DEFAULT_MANIFEST_URL}
          spellCheck={false}
        />

        <label className="mt-4 block text-[10px] font-black uppercase tracking-[0.28em] text-slate-400" htmlFor="manifest-proxy-input">
          Proxy Base URL
        </label>
        <input
          id="manifest-proxy-input"
          value={proxyBaseUrl}
          onChange={(event) => handleProxyBaseUrlChange(event.target.value)}
          disabled={isLoading}
          className="mt-3 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-200/50"
          placeholder="https://mirror.example.com"
          spellCheck={false}
        />

        <div className="mt-3 flex flex-wrap items-center gap-3">
          <button
            onClick={() => void load()}
            disabled={!manifestUrl.trim() || isLoading}
            className="border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-slate-950 transition disabled:cursor-not-allowed disabled:border-cyan-200/20 disabled:bg-cyan-200/20 disabled:text-cyan-100/55"
          >
            {isLoading ? "加载中..." : "加载 manifest"}
          </button>
          <div className="text-xs uppercase tracking-[0.18em] text-slate-500">Readonly Manifest Intake / No Install Transaction</div>
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[240px_minmax(0,1fr)]">
        <div className="border border-cyan-200/12 bg-black/30 p-3">
          <div className="mb-3 flex items-center justify-between text-[10px] font-black uppercase tracking-[0.28em] text-cyan-100/72">
            <span>Manifest Stats</span>
            <span>{manifest ? manifest.clients.length : 0}</span>
          </div>
          <div className="grid gap-2 border border-white/6 bg-black/20 p-3 text-[11px] uppercase tracking-[0.18em] text-slate-400">
            <div className="flex items-center justify-between">
              <span>Schema</span>
              <span className="text-cyan-100">{manifest?.schema_version ?? "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Clients</span>
              <span className="text-cyan-100">{manifest?.clients.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Assets</span>
              <span className="text-cyan-100">
                {manifest ? manifest.clients.reduce((sum, client) => sum + client.assets.length, 0) : 0}
              </span>
            </div>
          </div>
          <div className="mt-4 border border-amber-200/12 bg-black/25 p-3 text-xs leading-6 text-amber-100/86">
            仅展示 manifest 解析结果与资产元数据，不触发下载、校验或安装写入。
          </div>
        </div>

        <div className="border border-white/8 bg-black/25 p-3">
          <div className="mb-3 text-[10px] font-black uppercase tracking-[0.28em] text-slate-300/80">Client Releases</div>
          <div className="max-h-[420px] space-y-3 overflow-y-auto pr-1">
            {manifest ? (
              manifest.clients.map((client) => (
                <article key={`${client.client_id}-${client.channel}-${client.version}`} className="border border-white/8 bg-black/20 p-3">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <div className="text-xs font-black uppercase tracking-[0.24em] text-cyan-100">
                        {client.client_id}
                      </div>
                      <div className="mt-2 flex flex-wrap gap-2 text-[10px] font-black uppercase tracking-[0.22em] text-slate-400">
                        <span className="border border-white/8 bg-white/5 px-2 py-1">{client.channel}</span>
                        <span className="border border-white/8 bg-white/5 px-2 py-1">{client.version}</span>
                        <span className="border border-white/8 bg-white/5 px-2 py-1">{client.assets.length} assets</span>
                      </div>
                    </div>
                  </div>

                  <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-slate-300/84">
                    {client.release_notes || "No release notes."}
                  </p>

                  <div className="mt-4 space-y-2">
                    {client.assets.map((asset) => (
                      <div key={`${asset.platform}-${asset.asset_url}`} className="border border-cyan-200/10 bg-black/30 p-3 text-xs leading-6 text-slate-200">
                        <div className="flex flex-wrap items-center gap-2 text-[10px] font-black uppercase tracking-[0.22em] text-cyan-100/84">
                          <span>{asset.platform}</span>
                          <span className="text-slate-500">/</span>
                          <span>{formatAssetSize(asset.size)}</span>
                          <span className="text-slate-500">/</span>
                          <span>{asset.sha256.slice(0, 12)}</span>
                        </div>
                        <div className="mt-2 break-all text-slate-300/84">{asset.asset_url}</div>
                      </div>
                    ))}
                  </div>
                </article>
              ))
            ) : (
              <div className="border border-dashed border-white/10 bg-black/20 px-4 py-6 text-sm text-slate-400">
                输入 manifest 地址后可查看客户端发布、渠道和资产摘要。
              </div>
            )}
          </div>
        </div>
      </div>

      {error ? <div className="mt-4 text-sm text-red-300">{error}</div> : null}
    </section>
  );
}
