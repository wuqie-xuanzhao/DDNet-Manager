import { useCallback, useMemo, useState } from "react";
import { useEscToClose } from "./useEscToClose";

/// Popover 状态机：组合 `useState(open)` + Esc 关闭 + backdrop 点击关闭。
/// 替代各 popover 调用方手写 "state + useEscToClose + 渲染 backdrop div" 三件套，
/// 让 popover 行为收敛到一处，新增 popover 时只 spread backdropProps 即可。
///
/// 用法：
/// ```tsx
/// const { open, setOpen, backdropProps } = usePopoverState();
/// return (
///   <div className="relative">
///     <button onClick={() => setOpen((v) => !v)}>toggle</button>
///     {open && (
///       <>
///         <div className="fixed inset-0 z-40" {...backdropProps} />
///         <div className="absolute ...">popover 内容</div>
///       </>
///     )}
///   </div>
/// );
/// ```
///
/// 不渲染 backdrop div 本身——z-index 与定位由调用方决定（不同 popover 的层级不同，
/// 收敛到 hook 里反而僵化）。hook 只封装行为：open 状态、Esc 触发 close、
/// backdropProps（spread 到 backdrop div 上，onClick = close、aria-hidden 屏蔽 a11y）。
///
/// 多个 popover 同时 open 时各自管理状态，互不影响；当前项目所有 popover 是模态
/// backdrop 阻断的，互斥，无并发问题。
export interface PopoverState {
  open: boolean;
  setOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  /// 关闭（等价于 setOpen(false)，语义更清晰，便于调用方直接传给 onClose 回调）。
  close: () => void;
  /// spread 到 backdrop div 上：onClick 触发 close + aria-hidden 屏蔽读屏。
  /// onClick 引用稳定（close 来自空依赖 useCallback），不会触发 backdrop div
  /// 的 handler 无意义重绑。
  backdropProps: {
    onClick: () => void;
    "aria-hidden": true;
  };
}

export function usePopoverState(initialOpen = false): PopoverState {
  const [open, setOpen] = useState<boolean>(initialOpen);

  // review issue M1：close 用 useCallback 包装保证引用稳定。useEscToClose 内部
  // useEffect 依赖 [open, onClose]，传入 stable close 后只在 open 变化时重挂
  // listener，避免每次父组件 re-render 都 remove/addEventListener 一轮。
  const close = useCallback(() => setOpen(false), []);
  useEscToClose(open, close);

  const backdropProps = useMemo<PopoverState["backdropProps"]>(
    () => ({
      onClick: close,
      "aria-hidden": true
    }),
    [close]
  );

  return { open, setOpen, close, backdropProps };
}
