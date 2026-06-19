import { useEffect } from "react";

/// 弹层 Esc 关闭：open 为 true 时监听 document keydown，按 Escape 触发 onClose。
///
/// 不接管 open state（调用方继续用 useState 自己持有），也不处理 backdrop 点击
/// （由调用方 JSX 渲染 backdrop div + onClick 自行关闭），更不处理焦点 trap
/// （那是 Dialog 行为，Popover 不需要）。单职责，方便组合。
///
/// 使用示例：
/// ```tsx
/// const [open, setOpen] = useState(false);
/// useEscToClose(open, () => setOpen(false));
/// return (
///   <>
///     {open && <div className="fixed inset-0" onClick={() => setOpen(false)} />}
///     {open && <Popover>...</Popover>}
///   </>
/// );
/// ```
///
/// 若多个弹层同时 open（罕见），后挂的 listener 后触发——Escape 会同时关闭所有
/// 监听中的弹层。当前项目所有 popover 是模态 backdrop 阻断的，互斥，无此问题。
export function useEscToClose(open: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!open) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => {
      document.removeEventListener("keydown", handler);
    };
  }, [open, onClose]);
}
