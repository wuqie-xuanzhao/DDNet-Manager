import { useRef, useState } from "react";
import { analyzeCfgFile, loadWorkshopBinds } from "../../lib/tauri";
import type { CfgAnalysis, WorkshopBind } from "../../types";

const WORKSHOP_BINDS_URL = "https://ddrace.cn/data/binds.json";

export function BindsPanel() {
  const [cfgPath, setCfgPath] = useState("");
  const [analysis, setAnalysis] = useState<CfgAnalysis | null>(null);
  const [workshop, setWorkshop] = useState<WorkshopBind[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [isLoadingWorkshop, setIsLoadingWorkshop] = useState(false);
  const analyzeRequestIdRef = useRef(0);
  const workshopRequestIdRef = useRef(0);

  const handleCfgPathChange = (nextPath: string) => {
    analyzeRequestIdRef.current += 1;
    setCfgPath(nextPath);
    setAnalysis(null);
    setError(null);
    setIsAnalyzing(false);
  };

  const analyze = async () => {
    const nextPath = cfgPath.trim();
    if (!nextPath) {
      return;
    }

    const requestId = analyzeRequestIdRef.current + 1;
    analyzeRequestIdRef.current = requestId;

    setError(null);
    setAnalysis(null);
    setIsAnalyzing(true);

    try {
      const nextAnalysis = await analyzeCfgFile(nextPath);
      if (analyzeRequestIdRef.current !== requestId) {
        return;
      }
      setAnalysis(nextAnalysis);
    } catch (err) {
      if (analyzeRequestIdRef.current !== requestId) {
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (analyzeRequestIdRef.current === requestId) {
        setIsAnalyzing(false);
      }
    }
  };

  const loadWorkshop = async () => {
    const requestId = workshopRequestIdRef.current + 1;
    workshopRequestIdRef.current = requestId;

    setError(null);
    setWorkshop([]);
    setIsLoadingWorkshop(true);

    try {
      const nextWorkshop = await loadWorkshopBinds(WORKSHOP_BINDS_URL);
      if (workshopRequestIdRef.current !== requestId) {
        return;
      }
      setWorkshop(nextWorkshop);
    } catch (err) {
      if (workshopRequestIdRef.current !== requestId) {
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (workshopRequestIdRef.current === requestId) {
        setIsLoadingWorkshop(false);
      }
    }
  };

  return (
    <section className="rounded-[34px] border border-[var(--dm-border)] bg-white/82 p-6 text-[var(--dm-ink)] shadow-[0_24px_70px_rgba(47,52,64,0.10)] backdrop-blur-2xl">
      <h2 className="text-3xl font-black tracking-[-0.05em]">Binds 管理</h2>
      <p className="mt-2 text-sm font-semibold text-[#4f5663]">分析本地 cfg，并预览 Workshop 数据。</p>

      <div className="mt-5 rounded-[26px] bg-[var(--dm-soft)] p-4">
        <label className="block text-[11px] font-black tracking-[0.18em] text-[#59606d]" htmlFor="binds-cfg-input">
          cfg 文件路径
        </label>
        <input
          id="binds-cfg-input"
          value={cfgPath}
          onChange={(event) => handleCfgPathChange(event.target.value)}
          disabled={isAnalyzing}
          className="mt-3 h-12 w-full rounded-[18px] border border-[var(--dm-border)] bg-white px-4 text-sm font-semibold text-[var(--dm-ink)] outline-none transition placeholder:text-[#7b808c] focus:border-[var(--dm-ink)]/40 focus:ring-4 focus:ring-[var(--dm-ink)]/10"
          placeholder="C:/Users/User/AppData/Roaming/DDNet/settings_ddnet.cfg"
          spellCheck={false}
        />

        <div className="mt-4 flex flex-wrap items-center gap-3">
          <button
            onClick={() => void analyze()}
            disabled={!cfgPath.trim() || isAnalyzing}
            className="h-11 rounded-[18px] bg-[var(--dm-ink)] px-5 text-sm font-black text-white transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
          >
            {isAnalyzing ? "分析中" : "分析 cfg"}
          </button>
          <button
            onClick={() => void loadWorkshop()}
            disabled={isLoadingWorkshop}
            className="h-11 rounded-[18px] border border-[var(--dm-border)] bg-white px-5 text-sm font-black text-[var(--dm-ink)] transition hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:opacity-55"
          >
            {isLoadingWorkshop ? "加载中" : "加载 Workshop"}
          </button>
          <div className="text-xs font-bold text-[#59606d]">只读分析。</div>
        </div>
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
          <div className="mb-3 flex items-center justify-between text-xs font-black text-[#59606d]">
            <span>cfg 分析结果</span>
            <span>{analysis ? analysis.binds.length : 0}</span>
          </div>
          <div className="grid gap-2 rounded-[20px] bg-white/76 p-3 text-sm font-bold text-[#3d4350]">
            <div className="flex items-center justify-between">
              <span>bind 数量</span>
              <span>{analysis?.binds.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>unbind 数量</span>
              <span>{analysis?.unbinds.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>exec 引用</span>
              <span>{analysis?.execs.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>按键冲突</span>
              <span>{analysis?.conflicts.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>缺失的 exec 文件</span>
              <span className="text-[#8f2f2f]">{analysis?.missing_exec_targets.length ?? 0}</span>
            </div>
          </div>
          <pre className="dm-scroll mt-3 max-h-64 overflow-auto rounded-[20px] bg-white/78 p-3 text-xs leading-6 text-[#2f3440]">
            {JSON.stringify(analysis, null, 2)}
          </pre>
        </div>

        <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
          <div className="mb-3 flex items-center justify-between text-xs font-black text-[#59606d]">
            <span>Workshop 数据</span>
            <span>{Math.min(workshop.length, 5)}</span>
          </div>
          <pre className="dm-scroll max-h-64 overflow-auto rounded-[20px] bg-white/78 p-3 text-xs leading-6 text-[#2f3440]">
            {JSON.stringify(workshop.slice(0, 5), null, 2)}
          </pre>
        </div>
      </div>

      {analysis ? (
        <div className="mt-4 grid gap-4 xl:grid-cols-2">
          <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
            <div className="mb-3 text-xs font-black text-[#59606d]">按键冲突</div>
            <pre className="dm-scroll max-h-52 overflow-auto rounded-[20px] bg-white/78 p-3 text-xs leading-6 text-[#2f3440]">
              {JSON.stringify(analysis.conflicts, null, 2)}
            </pre>
          </div>
          <div className="rounded-[26px] bg-[var(--dm-soft)] p-4">
            <div className="mb-3 text-xs font-black text-[#8f2f2f]">缺失的 exec 文件</div>
            <pre className="dm-scroll max-h-52 overflow-auto rounded-[20px] bg-white/78 p-3 text-xs leading-6 text-[#2f3440]">
              {JSON.stringify(analysis.missing_exec_targets, null, 2)}
            </pre>
          </div>
        </div>
      ) : null}

      {error ? <div className="mt-4 rounded-2xl border border-[#b84a4a]/20 bg-[#b84a4a]/8 px-4 py-3 text-sm font-semibold text-[#8f2f2f]">{error}</div> : null}
    </section>
  );
}
