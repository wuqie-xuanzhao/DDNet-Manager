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
    <section className="dm-glass dm-cut-corner p-5">
      <div className="text-[11px] font-black uppercase tracking-[0.3em] text-cyan-200">Binds</div>
      <h2 className="mt-3 text-2xl font-black">Binds 管理</h2>
      <p className="mt-2 text-sm text-slate-300/80">分析本地 cfg，接入 Workshop，并通过安全事务写入。</p>

      <div className="mt-5 border border-white/10 bg-black/20 p-4">
        <label className="block text-[10px] font-black uppercase tracking-[0.28em] text-slate-400" htmlFor="binds-cfg-input">
          CFG Path
        </label>
        <input
          id="binds-cfg-input"
          value={cfgPath}
          onChange={(event) => handleCfgPathChange(event.target.value)}
          disabled={isAnalyzing}
          className="mt-3 w-full border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-200/50"
          placeholder="C:/Users/User/AppData/Roaming/DDNet/settings_ddnet.cfg"
          spellCheck={false}
        />

        <div className="mt-3 flex flex-wrap items-center gap-3">
          <button
            onClick={() => void analyze()}
            disabled={!cfgPath.trim() || isAnalyzing}
            className="border border-cyan-200 bg-cyan-200 px-4 py-2 text-sm font-black text-slate-950 transition disabled:cursor-not-allowed disabled:border-cyan-200/20 disabled:bg-cyan-200/20 disabled:text-cyan-100/55"
          >
            {isAnalyzing ? "分析中..." : "分析 cfg"}
          </button>
          <button
            onClick={() => void loadWorkshop()}
            disabled={isLoadingWorkshop}
            className="border border-white/10 bg-white/5 px-4 py-2 text-sm font-black text-slate-100 transition hover:border-cyan-200/40 disabled:cursor-not-allowed disabled:border-white/6 disabled:text-slate-500"
          >
            {isLoadingWorkshop ? "加载中..." : "加载 Workshop"}
          </button>
          <div className="text-xs uppercase tracking-[0.18em] text-slate-500">Readonly Analysis / Remote Bind Catalog</div>
        </div>
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <div className="border border-cyan-200/12 bg-black/30 p-3">
          <div className="mb-3 flex items-center justify-between text-[10px] font-black uppercase tracking-[0.28em] text-cyan-100/72">
            <span>CFG Analysis</span>
            <span>{analysis ? analysis.binds.length : 0}</span>
          </div>
          <div className="grid gap-2 border border-white/6 bg-black/20 p-3 text-[11px] uppercase tracking-[0.18em] text-slate-400">
            <div className="flex items-center justify-between">
              <span>Binds</span>
              <span className="text-cyan-100">{analysis?.binds.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Unbinds</span>
              <span className="text-cyan-100">{analysis?.unbinds.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Execs</span>
              <span className="text-cyan-100">{analysis?.execs.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Conflicts</span>
              <span className="text-cyan-100">{analysis?.conflicts.length ?? 0}</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Missing Exec</span>
              <span className="text-red-200">{analysis?.missing_exec_targets.length ?? 0}</span>
            </div>
          </div>
          <pre className="max-h-64 overflow-auto text-xs leading-6 text-cyan-100">
            {JSON.stringify(analysis, null, 2)}
          </pre>
        </div>

        <div className="border border-amber-200/12 bg-black/30 p-3">
          <div className="mb-3 flex items-center justify-between text-[10px] font-black uppercase tracking-[0.28em] text-amber-100/72">
            <span>Workshop Preview</span>
            <span>{Math.min(workshop.length, 5)}</span>
          </div>
          <pre className="max-h-64 overflow-auto text-xs leading-6 text-amber-100">
            {JSON.stringify(workshop.slice(0, 5), null, 2)}
          </pre>
        </div>
      </div>

      {analysis ? (
        <div className="mt-4 grid gap-4 xl:grid-cols-2">
          <div className="border border-white/8 bg-black/25 p-3">
            <div className="mb-3 text-[10px] font-black uppercase tracking-[0.28em] text-slate-300/80">Conflicts</div>
            <pre className="max-h-52 overflow-auto text-xs leading-6 text-slate-100">
              {JSON.stringify(analysis.conflicts, null, 2)}
            </pre>
          </div>
          <div className="border border-red-200/12 bg-black/25 p-3">
            <div className="mb-3 text-[10px] font-black uppercase tracking-[0.28em] text-red-200/80">Missing Exec Targets</div>
            <pre className="max-h-52 overflow-auto text-xs leading-6 text-red-100">
              {JSON.stringify(analysis.missing_exec_targets, null, 2)}
            </pre>
          </div>
        </div>
      ) : null}

      {error ? <div className="mt-4 text-sm text-red-300">{error}</div> : null}
    </section>
  );
}
