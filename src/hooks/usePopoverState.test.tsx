import { act, fireEvent, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { usePopoverState } from "./usePopoverState";

describe("usePopoverState", () => {
  it("默认 closed", () => {
    const { result } = renderHook(() => usePopoverState());
    expect(result.current.open).toBe(false);
  });

  it("initialOpen=true 初始为 open", () => {
    const { result } = renderHook(() => usePopoverState(true));
    expect(result.current.open).toBe(true);
  });

  it("setOpen 切换 open 状态", () => {
    const { result } = renderHook(() => usePopoverState());
    act(() => result.current.setOpen(true));
    expect(result.current.open).toBe(true);
    act(() => result.current.setOpen(false));
    expect(result.current.open).toBe(false);
  });

  it("setOpen 支持函数形式", () => {
    const { result } = renderHook(() => usePopoverState(true));
    act(() => result.current.setOpen((v) => !v));
    expect(result.current.open).toBe(false);
  });

  it("close 等价于 setOpen(false)", () => {
    const { result } = renderHook(() => usePopoverState(true));
    act(() => result.current.close());
    expect(result.current.open).toBe(false);
  });

  it("open 时按 Escape 触发 close", () => {
    const { result } = renderHook(() => usePopoverState(true));
    expect(result.current.open).toBe(true);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(result.current.open).toBe(false);
  });

  it("closed 时按 Escape 不触发任何 setState（无效输入）", () => {
    const { result } = renderHook(() => usePopoverState(false));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(result.current.open).toBe(false);
  });

  it("backdropProps.onClick 触发 close", () => {
    const { result } = renderHook(() => usePopoverState(true));
    act(() => result.current.backdropProps.onClick());
    expect(result.current.open).toBe(false);
  });

  it("backdropProps 含 aria-hidden=true 屏蔽读屏", () => {
    const { result } = renderHook(() => usePopoverState(true));
    expect(result.current.backdropProps["aria-hidden"]).toBe(true);
  });

  it("backdropProps.onClick 引用稳定（不随 re-render 变化）", () => {
    const { result, rerender } = renderHook(() => usePopoverState(true));
    const firstOnClick = result.current.backdropProps.onClick;
    rerender();
    expect(result.current.backdropProps.onClick).toBe(firstOnClick);
  });

  it("backdropProps 整体对象引用稳定（useMemo 包装，不随 re-render 重建）", () => {
    const { result, rerender } = renderHook(() => usePopoverState(true));
    const firstBackdropProps = result.current.backdropProps;
    rerender();
    expect(result.current.backdropProps).toBe(firstBackdropProps);
  });

  it("close 引用稳定", () => {
    const { result, rerender } = renderHook(() => usePopoverState(true));
    const firstClose = result.current.close;
    rerender();
    expect(result.current.close).toBe(firstClose);
  });

  it("多实例状态独立、Escape 同时关闭所有监听中的 popover", () => {
    const { result: a } = renderHook(() => usePopoverState(false));
    const { result: b } = renderHook(() => usePopoverState(false));
    act(() => a.current.setOpen(true));
    expect(a.current.open).toBe(true);
    expect(b.current.open).toBe(false);
    // Escape 同时关闭所有监听中的 popover（useEscToClose 文档已声明，属预期行为）
    fireEvent.keyDown(document, { key: "Escape" });
    expect(a.current.open).toBe(false);
    expect(b.current.open).toBe(false);
  });
});
