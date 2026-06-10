import { Component, type ErrorInfo, type ReactNode } from "react";
import { getErrorMessage } from "../lib/errors";

type ErrorBoundaryProps = {
  children: ReactNode;
};

type ErrorBoundaryState = {
  error: string | null;
};

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return { error: getErrorMessage(error) };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("DDNet Manager render error", error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <main className="grid min-h-screen place-items-center bg-[var(--dm-bg)] px-6 text-[var(--dm-ink)]">
          <section className="w-full max-w-xl rounded-[28px] border border-[var(--dm-border)] bg-[#161719]/86 p-8 shadow-[0_24px_80px_rgba(0,0,0,0.38)]">
            <div className="text-[11px] font-black uppercase tracking-[0.24em] text-red-400">Render fault</div>
            <h1 className="mt-3 text-3xl font-black">界面渲染失败</h1>
            <p className="mt-3 text-sm font-semibold leading-6 text-[var(--dm-muted-ink)]">{this.state.error}</p>
          </section>
        </main>
      );
    }

    return this.props.children;
  }
}
