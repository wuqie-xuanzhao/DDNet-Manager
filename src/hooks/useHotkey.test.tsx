import { act, fireEvent, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useHotkey, type HotkeyBinding } from "./useHotkey";

describe("useHotkey", () => {
  it("纯字符键触发 handler", () => {
    const handler = vi.fn();
    const bindings: HotkeyBinding[] = [{ key: "s", handler }];
    renderHook(() => useHotkey(bindings));
    act(() => {
      fireEvent.keyDown(document, { key: "s" });
    });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("key 大小写不敏感", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: "S", handler }]));
    act(() => {
      fireEvent.keyDown(document, { key: "s" });
    });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("Ctrl+组合键：要求 ctrl=true 时纯字符不触发", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: "s", ctrl: true, handler }]));
    act(() => {
      fireEvent.keyDown(document, { key: "s" }); // 无 Ctrl
    });
    expect(handler).not.toHaveBeenCalled();
    act(() => {
      fireEvent.keyDown(document, { key: "s", ctrlKey: true });
    });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("Ctrl+组合键：macOS Cmd（metaKey）也匹配", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: ",", ctrl: true, handler }]));
    act(() => {
      fireEvent.keyDown(document, { key: ",", metaKey: true });
    });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("Shift+组合键：要求 shift 与 event 一致", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: "s", shift: true, handler }]));
    act(() => {
      fireEvent.keyDown(document, { key: "S", shiftKey: true });
    });
    expect(handler).toHaveBeenCalledTimes(1);
    act(() => {
      fireEvent.keyDown(document, { key: "s" });
    });
    expect(handler).toHaveBeenCalledTimes(1); // 没增加
  });

  it("enabled=false 时不绑定 listener", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: "s", handler }], false));
    act(() => {
      fireEvent.keyDown(document, { key: "s" });
    });
    expect(handler).not.toHaveBeenCalled();
  });

  it("unmount 时移除 listener", () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useHotkey([{ key: "s", handler }]));
    unmount();
    act(() => {
      fireEvent.keyDown(document, { key: "s" });
    });
    expect(handler).not.toHaveBeenCalled();
  });

  it("多个 binding 按顺序匹配，命中第一个后停止", () => {
    const handler1 = vi.fn();
    const handler2 = vi.fn();
    renderHook(() =>
      useHotkey([
        { key: "a", handler: handler1 },
        { key: "a", handler: handler2 }
      ])
    );
    act(() => {
      fireEvent.keyDown(document, { key: "a" });
    });
    expect(handler1).toHaveBeenCalledTimes(1);
    expect(handler2).not.toHaveBeenCalled();
  });

  it("编辑框内纯字符键不触发，避免输入 \"1\" 误触发", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: "1", handler }]));
    const input = document.createElement("input");
    document.body.appendChild(input);
    act(() => {
      fireEvent.keyDown(input, { key: "1" });
    });
    expect(handler).not.toHaveBeenCalled();
    document.body.removeChild(input);
  });

  it("编辑框内 Ctrl+组合键仍触发（让 Ctrl+S 等在 input 内可用）", () => {
    const handler = vi.fn();
    renderHook(() => useHotkey([{ key: "s", ctrl: true, handler }]));
    const input = document.createElement("input");
    document.body.appendChild(input);
    act(() => {
      fireEvent.keyDown(input, { key: "s", ctrlKey: true });
    });
    expect(handler).toHaveBeenCalledTimes(1);
    document.body.removeChild(input);
  });
});
