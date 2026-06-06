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
    <section className="rounded-[34px] border border-[var(--dm-border)] bg-white/82 p-6 text-[var(--dm-ink)] shadow-[0_24px_70px_rgba(47,52,64,0.10)] backdrop-blur-2xl">
      <h2 className="text-3xl font-black tracking-[-0.05em]">Manifest 更新</h2>
      <p className="mt-2 text-sm font-semibold text-[#4f5663]">读取 manifest，查看客户端发布和资产摘要。</p>

      <div className="mt-5 rounded-[26px] bg-[var(--dm-soft)] p-4">
        <label className="block text-[11px] font-black tracking-[0.18em] text-[#59606d]" htmlFor="manifest-url-input">
          Manifest URL
        </label>
        <input
          id="manifest-url-input"
          value={manifestUrl}
          onChange={(event) => handleManifestUrlChange(event.target.value)}
          disabled={isLoading}
          className="mt-3 h-12 w-full rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
          placeholder={DEFAULT_MANIFEST_URL}
          spellCheck={false}
        />

        <label className="mt-4 block text-[11px] font-black tracking-[0.18em] text-[#59606d]" htmlFor="manifest-proxy-input">
          Proxy Base URL
        </label>
        <input
          id="manifest-proxy-input"
          value={proxyBaseUrl}
          onChange={(event) => handleProxyBaseUrlChange(event.target.value)}
          disabled={isLoading}
          className="mt-3 h-12 w-full rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
          placeholder="https://mirror.example.com"
          spellCheck={false}
        />

        <div className="mt-4 flex flex-wrap items-center gap-3">
          <button
            onClick={() => void load()}
            disabled={!manifestUrl.trim() || isLoading}
            className="h-11 rounded-[18px] bg-[var(--dm-ink)] px-5 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
          >
            {isLoading ? "加载中" : "加载 manifest"}
          </button>
          <div className="text-xs font-bold text-[#59606d]">只读解析，不执行安装。</div>
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[240px_minmax(0,1fr)]">
        <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
          <div className="mb-3 flex items-center justify-between text-xs font-black text-[#59606d]">
            <span>Manifest Stats</span>
            <span>{manifest ? manifest.clients.length : 0}</span>
          </div>
          <div className="grid gap-2 rounded-[20px] bg-white/76 p-3 text-sm font-bold text-[#3d4350]">
            <div className="flex items-center justify-between">
              <span>Schema</span>
              <span>{manifest?.schema_version ?? "-"}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Clients</span>
              <span>{manifest?.clients.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Assets</span>
              <span>{manifest ? manifest.clients.reduce((sum, client) => sum + client.assets.length, 0) : 0}</span>
            </div>
          </div>
        </div>

        <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
          <div className="mb-3 text-xs font-black text-[#59606d]">Client Releases</div>
          <div className="dm-scroll max-h-[420px] space-y-3 overflow-y-auto pr-1">
            {manifest ? (
              manifest.clients.map((client) => (
                <article key={`${client.client_id}-${client.channel}-${client.version}`} className="rounded-[20px] bg-white/78 p-4 text-[var(--dm-ink)]">
                  <div className="text-sm font-black">{client.client_id}</div>
                  <div className="mt-2 flex flex-wrap gap-2 text-xs font-black text-[#59606d]">
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{client.channel}</span>
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{client.version}</span>
                    <span className="rounded-full bg-[var(--dm-soft)] px-3 py-1">{client.assets.length} assets</span>
                  </div>
                  <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-[#3d4350]">
                    {client.release_notes || "No release notes."}
                  </p>
                  <div className="mt-4 space-y-2">
                    {client.assets.map((asset) => (
                      <div key={`${asset.platform}-${asset.asset_url}`} className="rounded-[18px] bg-[var(--dm-soft)] p-3 text-xs leading-6 text-[#3d4350]">
                        <div className="font-black text-[var(--dm-ink)]">
                          {asset.platform} / {formatAssetSize(asset.size)} / {asset.sha256.slice(0, 12)}
                        </div>
                        <div className="mt-1 break-all">{asset.asset_url}</div>
                      </div>
                    ))}
                  </div>
                </article>
              ))
            ) : (
              <div className="rounded-[20px] border border-dashed border-[var(--dm-border)] bg-white/64 px-4 py-6 text-sm font-semibold text-[#59606d]">
                输入 manifest 地址后可查看客户端发布、渠道和资产摘要。
              </div>
            )}
          </div>
        </div>
      </div>

      {error ? <div className="mt-4 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">{error}</div> : null}
    </section>
  );
}
