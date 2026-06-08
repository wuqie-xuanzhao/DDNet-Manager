import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

function BrokenView(): never {
  throw new Error("render exploded");
}

describe("ErrorBoundary", () => {
  it("renders a fallback instead of crashing the whole app", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);

    render(
      <ErrorBoundary>
        <BrokenView />
      </ErrorBoundary>
    );

    expect(screen.getByText("界面渲染失败")).toBeInTheDocument();
    expect(screen.getByText("render exploded")).toBeInTheDocument();
    consoleError.mockRestore();
  });
});
